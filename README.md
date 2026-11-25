# Yokio

[![Crates.io](https://img.shields.io/crates/v/yokio.svg)](https://crates.io/crates/yokio)
[![Documentation](https://docs.rs/yokio/badge.svg)](https://docs.rs/yokio)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A **callback-native asynchronous execution engine** for Rust. Fast, lightweight, and delightfully simple.

Yokio brings the battle-tested, single-threaded event loop model (proven by Node.js, Nginx, and Redis) to high-performance Rust systems engineering.

---

## 💡 The Sweet Spot: Why Developers Love Yokio

If you've ever wrestled with Rust's `async fn`, `Send + Sync` compiler walls, or complicated lifetimes across `.await` points, Yokio feels like fresh air:

* **⚡ Turn ANY Synchronous Code Into Non-Blocking Async (`yokio::blocking`)**:
  Want to use standard synchronous libraries like `std::fs`, `rusqlite`, `ureq`, `csv`, or image processing? Don't rewrite them as futures! Run them off-thread with `yokio::blocking()`. Yokio scales worker threads automatically and returns the result safely back to your event loop callback.
* **🤝 Plays Perfectly With Tokio or Any Async Stack**:
  You don't have to choose either-or! Run a Yokio event loop inside a Tokio background thread, or trigger Tokio tasks from Yokio. Yokio's cross-thread `Handle` lets background threads inject work into the loop without blocking.
* **🧠 No `async`/`await` Function Coloring**:
  In standard async Rust, making one function `async` forces every caller up the tree to become `async`. In Yokio, your code stays plain, regular, synchronous Rust functions.
* **🔒 Zero `Arc<Mutex<T>>` Tax**:
  Because the loop is single-threaded and shared-nothing, callbacks only need `'static` (NOT `Send`). Share state easily with plain `Rc<RefCell<T>>`—no locks, no deadlocks, and no atomic performance penalties.
* **🪶 Zero Heavy Dependencies**:
  Compiles in under 2 seconds. No giant dependency trees, no bloat. Pure standard library plus OS kernel bindings (`epoll` on Linux, `kqueue` on macOS, `WSAPoll` on Windows).
* **🎯 No Cancellation Hazards**:
  Futures can be dropped mid-execution at any `.await` point, leaving state corrupted. In Yokio, callbacks run to completion—once triggered, they finish cleanly.

---

## Quick Start

Add Yokio to your `Cargo.toml`:

```toml
[dependencies]
yokio = "0.1"
```

### Basic Example

```rust
use std::time::Duration;

#[yokio::main]
fn main() {
    // 1. One-shot task
    yokio::task(|| {
        println!("Hello from Yokio!");
    });

    // 2. Delayed timer
    yokio::after(Duration::from_millis(50), || {
        println!("50ms timer fired!");
    });

    // 3. Recurring timer
    let ticker = yokio::every(Duration::from_millis(20), {
        let mut count = 0;
        move || {
            count += 1;
            println!("Tick #{count}");
            if count >= 3 {
                yokio::stop(); // Terminates the event loop
            }
        }
    });

    // 4. Cancel a timer
    let cancelled = yokio::after(Duration::from_secs(10), || {
        unreachable!();
    });
    cancelled.cancel();
}
```

---

## Core Features

### 1. I/O Readiness Watching (`yokio::watch`)

Monitor any pollable file descriptor (sockets, pipes, stdin) using non-blocking OS readiness:

```rust
use std::io::Read;
use std::os::unix::net::UnixStream;
use yokio::io::{Interest, Mode};

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
                    println!("Received: {}", String::from_utf8_lossy(&buf[..n]));
                    yokio::stop();
                }
            }
        })
        .unwrap();

    let _ = writer.write_all(b"Hello from socket!");
}
```

### 2. Elastic Blocking Pool (`yokio::blocking`)

Run synchronous, blocking, or CPU-intensive work on dedicated worker threads without stalling the event loop:

```rust
use std::time::Duration;

#[yokio::main]
fn main() {
    yokio::blocking(
        || {
            // Runs on an off-thread worker
            std::thread::sleep(Duration::from_millis(100));
            42 * 2
        },
        |result| {
            // Result delivered safely back to the event loop callback
            println!("Computation completed: {result}");
            yokio::stop();
        },
    );
}
```

### 3. Cross-Thread Interaction (`Handle`)

Inject tasks into a running event loop from background threads:

```rust
#[yokio::main]
fn main() {
    let handle = yokio::handle();

    std::thread::spawn(move || {
        handle.post(|| {
            println!("Injected from an external background thread!");
            yokio::stop();
        }).unwrap();
    });
}
```

### 4. Asynchronous Filesystem (`features = ["fs"]`)

Offload blocking disk I/O to the elastic worker pool with zero async machinery:

```rust
use yokio::fs;

#[yokio::main]
fn main() {
    fs::read_to_string("config.toml", |result| {
        match result {
            Ok(content) => println!("Config: {content}"),
            Err(e) => eprintln!("Read failed: {e}"),
        }
    });
}
```

---

## Event Loop Execution Cycle

Each tick of Yokio's event loop executes in a deterministic normative phase order:

```text
 1. Update cached clock (yokio::now())
 2. Expire Timing Wheel slots (O(1) amortized)
 3. Drain Cross-Thread Inbox messages
 4. Run ready tasks FIFO (bounded for fairness)
 5. Drain Graveyard (deferred drops outside borrows)
 6. Check liveness / stop requests
 7. Sleep in OS Selector (epoll) with computed timeout
 8. Dispatch I/O readiness events
```

---

## Comparison: Yokio vs Tokio vs libuv

| Feature | Yokio | Tokio | libuv / Node.js |
|:---|:---|:---|:---|
| **Execution Model** | Run-to-completion Callbacks | Future State Machines | Callbacks |
| **Threading** | Loop-per-thread (`Rc<RefCell>`) | Multi-thread Work-stealing | Single-thread (`uv_loop`) |
| **Closure Allocations** | **0** (inline $\le 48$ B) | Boxed Future / Inline frame | V8 Heap Allocation |
| **Timers** | Hierarchical Wheel ($O(1)$) | Hierarchical Wheel ($O(1)$) | Binary Min-Heap ($O(\log n)$) |
| **Dependencies** | **0 external crates** (libc only) | Many crates | C library |
| **Blocking Pool** | Elastic (up to 256 threads) | Elastic (up to 512 threads) | Static (default 4 threads) |

---

## License

Licensed under the MIT License ([LICENSE](LICENSE) or <https://opensource.org/licenses/MIT>).
