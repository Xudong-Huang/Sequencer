#[cfg_attr(test, macro_use)]
extern crate may;

use std::sync::Arc;
use std::ops::{Deref, DerefMut};
use std::collections::LinkedList;
use std::cell::{Cell, UnsafeCell};
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};

use may::sync::{AtomicOption, Blocker, Mutex};

// save global seq info
// #[derive(Debug)]
struct Inner<T> {
    // current sequence number used by all Sequencer instances
    cur_seq: AtomicUsize,
    // track how many Sequencer instances created
    global_seq: Cell<usize>,
    // waiter list
    waiter_list: Mutex<LinkedList<Arc<AtomicOption<Arc<Blocker>>>>>,
    // the data
    data: UnsafeCell<T>,
}

/// `Seq` is a kind of sync primitive that the resource can be accessed only in
/// a sequential order by `Sequencer` instances that created by its `next` method
// #[derive(Debug)]
pub struct Seq<T> {
    inner: Arc<Inner<T>>,
}

impl<T> UnwindSafe for Seq<T> {}
impl<T> RefUnwindSafe for Seq<T> {}
unsafe impl<T: Send> Send for Seq<T> {}
unsafe impl<T: Send> Sync for Seq<T> {}

impl<T> From<T> for Seq<T> {
    /// Creates a new Sequencer in an unlocked state ready for use.
    /// This is equivalent to [`Sequencer::new`].
    ///
    /// [`Sequencer::new`]: #method.new
    fn from(t: T) -> Self {
        Seq::new(t)
    }
}

impl<T: Default> Default for Seq<T> {
    /// Creates a `Sequencer<T>`, with the `Default` value for T.
    fn default() -> Self {
        Seq::new(Default::default())
    }
}

impl<T> Seq<T> {
    /// Creates a `Seq` object
    pub fn new(t: T) -> Seq<T> {
        Seq {
            inner: Arc::new(Inner {
                cur_seq: AtomicUsize::new(0),
                global_seq: Cell::new(0),
                waiter_list: Mutex::new(LinkedList::new()),
                data: UnsafeCell::new(t),
            }),
        }
    }

    /// create the next sequencer instance.
    ///
    /// the new instance `lock` would unblock in the order of calling `next` method.
    pub fn next(&self) -> Sequencer<T> {
        let waiter = Arc::new(AtomicOption::none());
        let mut list_lock = self.inner.waiter_list.lock().unwrap();
        list_lock.push_back(waiter.clone());
        let seq_num = self.inner.global_seq.get();
        self.inner.global_seq.set(seq_num.wrapping_add(1));
        Sequencer {
            inner: self.inner.clone(),
            local_seq: seq_num,
            waiter,
        }
    }
}

/// `Sequencer` can be used to access the resource by calling 'lock' method.
/// The `lock` will return only if its previous `Sequencer` instance get "released".
/// A Sequencer is released if `lock` returned AND the returned `SeqGuard` got dropped
// #[derive(Debug)]
pub struct Sequencer<T> {
    inner: Arc<Inner<T>>,
    waiter: Arc<AtomicOption<Arc<Blocker>>>,
    local_seq: usize,
}

impl<T> UnwindSafe for Sequencer<T> {}
impl<T> RefUnwindSafe for Sequencer<T> {}
unsafe impl<T: Send> Send for Sequencer<T> {}

impl<T> Sequencer<T> {
    /// wait for the Sequencer instance
    ///
    /// the `lock` would block until the previous sequencer instance get released.
    /// A Sequencer is released if its `lock` returned AND the returned `SeqGuard` got dropped
    pub fn lock(mut self) -> SeqGuard<T> {
        use std::mem;

        self.wait();
        let g = SeqGuard {
            // drop inner manually
            inner: mem::replace(&mut self.inner, unsafe { mem::uninitialized() }),
        };
        // drop waiter manually
        let _ = mem::replace(&mut self.waiter, unsafe { mem::uninitialized() });
        // don't need to run the drop for Sequencer
        mem::forget(self);
        g
    }

    #[inline]
    fn wait(&self) {
        if self.local_seq == self.inner.cur_seq.load(Ordering::Acquire) {
            return;
        }

        // create a blocker that ignore the cancel signal
        let waiter = Arc::new(Blocker::new(true));
        self.waiter.swap(waiter.clone(), Ordering::Release);
        // recheck
        if self.local_seq == self.inner.cur_seq.load(Ordering::Acquire) {
            return;
        }

        // wait until got triggered
        waiter.park(None).expect("sequencer lock internal error");
    }
}

impl<T> Drop for Sequencer<T> {
    /// drop the Sequencer
    ///
    /// this will unblock the next Sequencer `lock` if it's not consumed by `lock`
    fn drop(&mut self) {
        self.wait();
        let _ = SeqGuard {
            inner: self.inner.clone(),
        };
    }
}

/// `SeqGuard` is something like `MutexGuard` that can be `deref` to the data
/// and the `drop` method would unblock the next `Sequencer` instance.
// #[derive(Debug)]
pub struct SeqGuard<T> {
    inner: Arc<Inner<T>>,
}

unsafe impl<T: Sync> Sync for SeqGuard<T> {}

impl<T> Deref for SeqGuard<T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.inner.data.get() }
    }
}

impl<T> DerefMut for SeqGuard<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.inner.data.get() }
    }
}

impl<T> Drop for SeqGuard<T> {
    /// drop the SeqGuard
    ///
    /// this will unblock the next Sequencer `lock`
    fn drop(&mut self) {
        let mut waiter_list = self.inner.waiter_list.lock().unwrap();
        // update the cur_seq
        self.inner.cur_seq.fetch_add(1, Ordering::Release);
        // remove self from the wait list
        assert_eq!(waiter_list.pop_front().is_some(), true);
        waiter_list
            .front()
            .map(|w| w.take(Ordering::Acquire).map(|w| w.unpark()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sanity() {
        let seq = Seq::new(0);
        let s1 = seq.next();
        let s2 = seq.next();
        let s3 = seq.next();
        {
            let g1 = s1.lock();
            assert_eq!(*g1, 0);
        }
        drop(s2);
        {
            // g1 must dropped first
            let g3 = s3.lock();
            assert_eq!(*g3, 0);
        }
    }

    #[test]
    fn test_seq() {
        let seq = Seq::new(0);
        may::coroutine::scope(|scope| {
            for i in 0..1000 {
                let s = seq.next();
                go!(scope, move || {
                    let mut g = s.lock();
                    assert_eq!(*g, i);
                    *g += 1;
                });
            }
        })
    }
}
