//! Sample demonstrating Yokio's complete execution engine:
//! - Entry macro: `#[yokio::main]`
//! - Run-to-completion callbacks: `yokio::task`, `yokio::after`, `yokio::every`
//! - I/O readiness watching: `yokio::watcher` on non-blocking sockets
//! - Elastic blocking thread pool: `yokio::blocking`

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;
use yokio::io::{Interest, Mode};

#[yokio::main]
fn main() {
    println!("=== Yokio Execution Engine Showcase ===");

    // 1. Scheduled run-to-completion tasks
    yokio::task(|| {
        println!("[Task] First FIFO task running on loop thread!");
    });

    // 2. Timers
    yokio::after(Duration::from_millis(30), || {
        println!("[Timer] 30ms timer fired!");
    });

    let _ticker = yokio::every(Duration::from_millis(15), {
        let mut count = 0;
        move || {
            count += 1;
            println!("[Ticker] Recurring tick #{count}");
            if count >= 3 {
                println!("[Ticker] Reached 3 ticks -> requesting stop!");
                yokio::stop();
            }
        }
    });

    // 3. I/O Readiness: Watch a non-blocking TCP socket
    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get local addr");
    let client = TcpStream::connect(addr).expect("failed to connect");
    let (server, _) = listener.accept().expect("failed to accept");

    server.set_nonblocking(true).unwrap();
    client.set_nonblocking(true).unwrap();

    let mut server_clone = server.try_clone().unwrap();
    let _watch = yokio::watcher(&server)
        .interest(Interest::READABLE)
        .mode(Mode::Edge)
        .start(move |ready| {
            if ready.is_readable() {
                let mut buf = [0u8; 32];
                if let Ok(n) = server_clone.read(&mut buf) {
                    if n > 0 {
                        let msg = String::from_utf8_lossy(&buf[..n]);
                        println!("[I/O Watcher] Received message over socket: {msg}");
                    }
                }
            }
        })
        .expect("failed to register watcher");

    // Send a message over the socket from a background thread
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        let mut client = client;
        let _ = client.write_all(b"Hello from socket!");
    });

    // 4. Elastic Blocking Pool: Run heavy/blocking work off the loop
    yokio::blocking(
        || {
            // This runs in a worker thread without blocking the event loop!
            std::thread::sleep(Duration::from_millis(20));
            42 * 10
        },
        |result| {
            // Result delivered safely back to the event loop thread
            println!("[Blocking Pool] Off-thread computation completed with result: {result}");
        },
    );
}
