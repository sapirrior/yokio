//! Unix self-pipe doorbell for cross-thread wakeups on macOS and BSD.

#![allow(unsafe_code)]

use crate::sys::types::RawFd;
use std::io;

/// Cross-thread doorbell primitive signaling sleeping event loops via non-blocking self-pipe.
pub(crate) struct Doorbell {
    read_fd: RawFd,
    write_fd: RawFd,
}

impl Doorbell {
    /// Creates a new non-blocking CLOEXEC self-pipe.
    pub(crate) fn new() -> io::Result<Self> {
        let mut fds = [0 as RawFd; 2];
        // SAFETY: fds has capacity for 2 RawFd
        let res = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }

        let read_fd = fds[0];
        let write_fd = fds[1];

        // Configure non-blocking and close-on-exec on both ends
        for &fd in &[read_fd, write_fd] {
            // SAFETY: fd is valid open descriptor
            unsafe {
                let flags = libc::fcntl(fd, libc::F_GETFL, 0);
                if flags >= 0 {
                    let _ = libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
                }
                let fd_flags = libc::fcntl(fd, libc::F_GETFD, 0);
                if fd_flags >= 0 {
                    let _ = libc::fcntl(fd, libc::F_SETFD, fd_flags | libc::FD_CLOEXEC);
                }
            }
        }

        Ok(Self { read_fd, write_fd })
    }

    /// Signals the doorbell by writing a single byte to the write end.
    pub(crate) fn ring(&self) {
        let byte: u8 = 1;
        // SAFETY: self.write_fd is valid, buffer is valid byte pointer
        let _ = unsafe {
            libc::write(
                self.write_fd,
                std::ptr::addr_of!(byte).cast::<libc::c_void>(),
                1,
            )
        };
    }

    /// Drains the doorbell by reading all pending bytes from the read end.
    pub(crate) fn drain(&self) {
        let mut buf = [0u8; 64];
        loop {
            // SAFETY: self.read_fd is non-blocking, buf is valid
            let n = unsafe {
                libc::read(
                    self.read_fd,
                    buf.as_mut_ptr().cast::<libc::c_void>(),
                    buf.len(),
                )
            };
            if n <= 0 {
                break;
            }
        }
    }

    /// Borrow the underlying raw read file descriptor registered with kqueue.
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.read_fd
    }
}

impl Drop for Doorbell {
    fn drop(&mut self) {
        // SAFETY: read_fd and write_fd are owned open file descriptors
        unsafe {
            libc::close(self.read_fd);
            libc::close(self.write_fd);
        }
    }
}
