//! Integration tests for I/O readiness watching.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use yokio::io::{Interest, Mode};
use yokio::Runtime;

#[test]
fn watch_socket_readable_event() {
    let rt = Runtime::new().expect("failed to create runtime");
    let _guard = rt.enter();

    let (reader, mut writer) = UnixStream::pair().expect("failed to create unix socketpair");
    reader
        .set_nonblocking(true)
        .expect("failed to set nonblocking");
    writer
        .set_nonblocking(true)
        .expect("failed to set nonblocking");

    let mut reader_clone = reader.try_clone().expect("failed to clone reader");

    let read_fired = Arc::new(AtomicBool::new(false));
    let read_fired_clone = Arc::clone(&read_fired);

    let watch = yokio::watcher(&reader)
        .interest(Interest::READABLE)
        .mode(Mode::Edge)
        .start(move |ready| {
            if ready.is_readable() {
                read_fired_clone.store(true, Ordering::SeqCst);
                let mut buf = [0u8; 16];
                let _ = reader_clone.read(&mut buf);
                yokio::stop();
            }
        })
        .expect("failed to watch reader");

    // Write a byte to the socket
    writer.write_all(b"x").expect("failed to write to socket");

    rt.run().expect("run failed");

    assert!(read_fired.load(Ordering::SeqCst));
    assert!(watch.cancel());
}
