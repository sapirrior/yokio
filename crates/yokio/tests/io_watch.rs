//! Integration tests for I/O readiness watching.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use yokio::io::{Interest, Mode};
use yokio::Runtime;

#[test]
fn watch_socket_readable_event() {
    let rt = Runtime::new().expect("failed to create runtime");
    let _guard = rt.enter();

    let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get addr");
    let client = TcpStream::connect(addr).expect("failed to connect");
    let (server, _) = listener.accept().expect("failed to accept");

    server
        .set_nonblocking(true)
        .expect("failed to set nonblocking");
    client
        .set_nonblocking(true)
        .expect("failed to set nonblocking");

    let mut server_clone = server.try_clone().expect("failed to clone reader");

    let read_fired = Arc::new(AtomicBool::new(false));
    let read_fired_clone = Arc::clone(&read_fired);

    let watch = yokio::watcher(&server)
        .interest(Interest::READABLE)
        .mode(Mode::Edge)
        .start(move |ready| {
            if ready.is_readable() {
                read_fired_clone.store(true, Ordering::SeqCst);
                let mut buf = [0u8; 16];
                let _ = server_clone.read(&mut buf);
                yokio::stop();
            }
        })
        .expect("failed to watch reader");

    // Write a byte to the socket
    let mut client = client;
    client.write_all(b"x").expect("failed to write to socket");

    rt.run().expect("run failed");

    assert!(read_fired.load(Ordering::SeqCst));
    assert!(watch.cancel());
}
