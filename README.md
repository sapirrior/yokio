# Yokio

[![Crates.io](https://img.shields.io/crates/v/yokio.svg)](https://crates.io/crates/yokio)
[![Docs.rs](https://docs.rs/yokio/badge.svg)](https://docs.rs/yokio)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A callback-native asynchronous execution engine for Rust.

Yokio owns an event loop and runs plain callbacks to completion. It fires timers, dispatches OS readiness events, coordinates work between threads without blocking the loop, and runs blocking jobs on an elastic worker pool. It does not use `async`/`await`, `Future`, or `Waker`.

## Contents

- [Design](#design)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Usage](#usage)
  - [Tasks and timers](#tasks-and-timers)
  - [Cancellation](#cancellation)
  - [Blocking work](#blocking-work)
  - [I/O readiness](#io-readiness)
  - [Working across threads](#working-across-threads)
  - [Embedding the runtime](#embedding-the-runtime)
  - [Testing](#testing)
- [API overview](#api-overview)
- [Feature flags](#feature-flags)
- [Platform support](#platform-support)
- [Status](#status)
- [License](#license)

## Design

- **Callbacks, not futures.** Work is expressed as closures. Once a callback starts, it runs to completion before the loop moves on.
- **One loop per thread.** A `Runtime` is single-threaded (`!Send`, `!Sync`). Callbacks passed to it only need to be `'static`, not `Send`, so they can share state through `Rc<RefCell<T>>` instead of `Arc<Mutex<T>>`.
- **Blocking work stays off the loop.** Jobs submitted with `yokio::blocking` run on a worker pool. Their result is delivered back to a callback on the loop thread.
- **Thread-safe entry point.** A `Handle` is `Send + Sync` and can be cloned to other threads to inject work into a running loop.

Yokio takes its async model from the event-loop style popularised by Node.js. It is an independent Rust implementation with its own runtime, scheduler, timer wheel, and I/O layer.

## Installation

```toml
[dependencies]
yokio = "0.1"
```

To enable optional features:

```toml
[dependencies]
yokio = { version = "0.1", features = ["fs"] }
```

## Quick start

```rust
use std::time::Duration;

#[yokio::main]
fn main() {
    // Run once on the next pass of the loop.
    yokio::task(|| println!("hello from the event loop"));

    // Run once after a delay.
    yokio::after(Duration::from_millis(50), || {
        println!("50 ms elapsed");
    });

    // Run repeatedly, then stop the loop.
    let mut ticks = 0;
    let _ticker = yokio::every(Duration::from_millis(20), move || {
        ticks += 1;
        println!("tick {ticks}");
        if ticks == 3 {
            yokio::stop();
        }
    });
}
```

`#[yokio::main]` is applied to a regular, non-`async` `fn main`. It creates the runtime, runs your body, and drives the event loop until it is idle or `yokio::stop()` is called.

## Usage

### Tasks and timers

| Function | Purpose |
|:--|:--|
| `yokio::task(f)` | Schedule a one-shot callback on the current thread's loop. |
| `yokio::after(delay, f)` | Schedule a one-shot timer. |
| `yokio::every(period, f)` | Schedule a recurring timer. |
| `yokio::now()` | The `Instant` cached at the start of the current tick. |
| `yokio::stop()` | Ask the runtime to stop after the current tick completes. |

`task` and `after` take an `FnOnce() + 'static`. `every` takes an `FnMut() + 'static`.

### Cancellation

`task`, `after`, and `every` return a `Task` or `Timer` handle. Both implement the `Cancel` trait.

```rust
use std::time::Duration;

#[yokio::main]
fn main() {
    let timer = yokio::after(Duration::from_secs(10), || {
        println!("this never runs");
    });
    timer.cancel();
}
```

`CancelOnDrop` wraps a handle so the work is cancelled when the wrapper is dropped.

### Blocking work

`yokio::blocking(job, done)` runs `job` on the worker pool and calls `done` with its result on the event loop.

```rust
use std::time::Duration;

#[yokio::main]
fn main() {
    yokio::blocking(
        || {
            // Runs on a worker thread.
            std::thread::sleep(Duration::from_millis(100));
            42 * 2
        },
        |result| {
            // Runs on the event loop.
            println!("result: {result}");
            yokio::stop();
        },
    );
}
```

The job must be `Send + 'static` and so must its return value. The `done` callback only needs to be `'static`.

Use `yokio::blocking_detached(job)` when you do not need a completion callback.

### I/O readiness

`yokio::watch` registers a callback that fires when an I/O source becomes ready. `yokio::watcher` returns a `WatchBuilder` for configuring interest and trigger mode before starting the watch.

```rust
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use yokio::{Interest, Mode};

#[yokio::main]
fn main() {
    let (mut reader, mut writer) = UnixStream::pair().unwrap();
    reader.set_nonblocking(true).unwrap();

    let _watch = yokio::watcher(&reader)
        .interest(Interest::READABLE)
        .mode(Mode::Edge)
        .start(move |ready| {
            if ready.is_readable() {
                let mut buf = [0u8; 64];
                if let Ok(n) = reader.read(&mut buf) {
                    println!("received: {}", String::from_utf8_lossy(&buf[..n]));
                    yokio::stop();
                }
            }
        })
        .unwrap();

    writer.write_all(b"hello").unwrap();
}
```

Sources must implement `AsSource`.

### Working across threads

`yokio::handle()` returns a cloneable `Handle` for the current runtime. A `Handle` can be sent to other threads to inject work into the loop or control it.

```rust
#[yokio::main]
fn main() {
    let handle = yokio::handle();

    std::thread::spawn(move || {
        handle
            .post(|| {
                println!("running on the event loop");
                yokio::stop();
            })
            .unwrap();
    });
}
```

A `KeepAlive` token prevents the loop from exiting while it is held, which is useful when work will arrive later from another thread.

### Embedding the runtime

The attribute macro is a convenience. You can also create and drive a `Runtime` yourself.

```rust
use yokio::Runtime;

fn main() -> yokio::Result<()> {
    let rt = Runtime::new()?;

    rt.task(|| println!("hello"));

    rt.run() // runs until idle or stopped
}
```

| Method | Purpose |
|:--|:--|
| `Runtime::new()` | Create a runtime with default capacity. |
| `run()` | Drive the loop until idle or stopped. |
| `run_with(main)` | Run a root closure inside the runtime and drive the loop to completion. |
| `tick(max_wait)` | Run a single loop iteration and return a `TickStatus`. Useful for integrating with another loop. |
| `enter()` | Make the runtime active on the current thread through an RAII `EnterGuard`. |
| `handle()` | Get a thread-safe `Handle`. |

`Runtime` also exposes `task`, `after`, `every`, `watch`, `watcher`, `blocking`, and `blocking_detached` as methods.

### Testing

`#[yokio::test]` runs a test function inside a Yokio runtime. The `test-util` feature flag is available for test helpers.

```rust
#[yokio::test]
fn timer_fires() {
    yokio::after(std::time::Duration::from_millis(10), || yokio::stop());
}
```

## API overview

| Module | Contents |
|:--|:--|
| `yokio` | Top-level functions: `task`, `after`, `every`, `watch`, `watcher`, `blocking`, `blocking_detached`, `handle`, `now`, `stop`. Attribute macros `main` and `test`. |
| `yokio::runtime` | `Runtime`, `Handle`, `KeepAlive`, `EnterGuard`, `TickStatus`. |
| `yokio::task` | Task scheduling primitives, storage slabs, callback erasure, and the cancellation types `Task`, `Timer`, `Cancel`, `CancelOnDrop`. |
| `yokio::time` | Clock and timing wheel. |
| `yokio::io` | I/O readiness monitoring: `Watch`, `WatchBuilder`, `Interest`, `Mode`, `Ready`, `AsSource`. |
| `yokio::blocking` | The elastic blocking pool and the `Blocking` handle for an in-flight job. |
| `yokio::error` | `Error`, `Result`, `JobPanic`, `PanicInfo`, `CallbackKind`. |

Full API documentation is on [docs.rs](https://docs.rs/yokio).

## Feature flags

| Flag | Description |
|:--|:--|
| `fs` | Filesystem helpers that run blocking disk I/O on the worker pool and deliver results to callbacks. |
| `test-util` | Utilities for testing code that runs on Yokio. |

No features are enabled by default.

## Platform support

Yokio uses the operating system's readiness API for I/O: epoll on Linux, kqueue on macOS, and WSAPoll on Windows. It depends on `libc` and `windows-sys` for the platform bindings, and on `yokio-macros` for the attribute macros.

## License

Licensed under the [MIT License](LICENSE).
