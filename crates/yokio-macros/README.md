# yokio-macros

Zero-dependency procedural macros for [Yokio](https://crates.io/crates/yokio), the callback-native asynchronous execution engine for Rust.

Provides:
- `#[yokio::main]`: Boots an event loop runtime and executes the synchronous entry function.
- `#[yokio::test]`: Runs unit or integration tests inside an isolated Yokio runtime.

## Zero Dependencies

Unlike typical proc-macro crates that pull in `syn`, `quote`, and dozens of transitive dependencies, `yokio-macros` is built purely on Rust's built-in `proc_macro` standard library. This ensures instantaneous compile times and zero supply-chain risk.

## Usage

```rust
#[yokio::main]
fn main() {
    yokio::task(|| {
        println!("Hello from Yokio!");
    });
}
```

Or for tests:

```rust
#[yokio::test]
fn my_test() {
    yokio::task(|| {
        assert!(true);
    });
}
```

## License

MIT OR Apache-2.0
