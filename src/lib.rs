#[cfg_attr(test, macro_use)]
extern crate may;

use std::sync::Arc;
use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::Ordering;
use std::collections::LinkedList;
use std::panic::{RefUnwindSafe, UnwindSafe};

use may::sync::{AtomicOption, Blocker, Mutex};

// save global seq info
// #[derive(Debug)]
struct Inner<T> {
    // current sequence number used by all Sequencer instances
    cur_seq: usize,
    // track how many Sequencer instances created
    global_seq: usize,
    // waiter list
    waiter_list: Mutex<LinkedList<Arc<AtomicOption<Blocker>>>>,
    // the data
    data: UnsafeCell<T>,
}

// impl<T> Inner<T> {
//     // get the internal data mut ref
//     #[inline]
//     fn get_data_mut(&mut self) -> &mut T {
//         unsafe { &mut *self.data.get() }
//     }
// }

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
    /// Creates a new Sequencer in an init state.
    ///
    /// the `wait` on it will always return immediately.
    pub fn new(t: T) -> Seq<T> {
        Seq {
            inner: Arc::new(Inner {
                cur_seq: 0,
                global_seq: 0,
                waiter_list: Mutex::new(LinkedList::new()),
                data: UnsafeCell::new(t),
            }),
        }
    }

    /// create a new sequencer instance.
    ///
    /// the new instance `lock` would unblock in the order of calling `next` method.
    pub fn next(&self) -> Sequencer<T> {
        unimplemented!()
        // let mut lock = self.global_sequence.lock().unwrap();
        // *lock += 1;
        // Sequencer {
        //     cur_sequence: self.cur_sequence.clone(),
        //     global_sequence: self.global_sequence.clone(),
        //     local_sequence: *lock,
        //     data: self.data.clone(),
        // }
    }
}

// #[derive(Debug)]
pub struct Sequencer<T> {
    inner: Arc<Inner<T>>,
    waiter: Arc<AtomicOption<Blocker>>,
    local_seq: usize,
}

impl<T> UnwindSafe for Sequencer<T> {}
impl<T> RefUnwindSafe for Sequencer<T> {}
unsafe impl<T: Send> Send for Sequencer<T> {}

impl<T> Sequencer<T> {
    /// wait for the Sequencer instance
    ///
    /// the `wait` would block until the previous cloned instance `release`.
    /// if the sequencer instance is already `release`ed, it will panic.
    pub fn lock(self) -> SeqGuard<T> {
        // if self.local_sequence == self.cur_sequence.load(Ordering::Acquire) {
        //     return self.get_data();
        // }

        unimplemented!()
    }

    /// release the current sequencer, unblock the next sequencer instance wait on it
    ///
    /// must be called in a ready state which is after `wait` or created via `new`
    /// or it will panic
    /// after call `release`, call `wait` on the same sequencer instance will panic.
    /// You must call this method explicitly before drop, or it will block
    /// other sequencer instances forever. Multiple `release` on the same sequencer
    /// instance take on effect.
    pub fn release(&mut self) {
        unimplemented!()
    }
}

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
    fn drop(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sanity() {
        let seq = Seq::new(0);
        let s1 = seq.next();
        let s2 = seq.next();
        {
            let g1 = s1.lock();
            assert_eq!(*g1, 0);
        }
        {
            // g1 must dropped first
            let g2 = s2.lock();
            assert_eq!(*g2, 0);
        }
    }

    #[test]
    fn test_seq() {
        let seq = Seq::new(0);
        for i in 0..10 {
            let s = seq.next();
            go!(move || {
                let g = s.lock();
                assert_eq!(*g, i);
            });
        }
    }
}
