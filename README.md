# Sequencer

Rust sequencer sync primitive implemented on top of [may](https://github.com/Xudong-Huang/may)

## Usage

add sequencer crate to your `Cargo.toml`:

```toml
[dependencies]
sequencer = { git = "https://github.com/Xudong-Huang/Sequencer.git" }
```

1. create the `Seq<T>` instance

```rust
use sequencer::Seq;
let seq = Seq::new(0);
```

2. create the `Sequencer<T>` instance
```rust
let sequencer = seq.next();
```

3. wait for the `Sequencer<T>` and got a `SeqGuard<T>` instance.
 When `SeqGuard<T>` is dropped, it will trigger the next sequencer.
```rust
let mut seq_guard = sequencer.lock();
*seq_guard += 1;
```

## Example
```rust,no_run
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
```

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

