//! Linux eventfd doorbell for cross-thread wakeups.

#![allow(unsafe_code)]

use std::io;
use std::os::fd::{AsRawFd, RawFd};

/// Cross-thread doorbell primitive signaling sleeping event loops via `eventfd`.
pub(crate) struct Doorbell {
    fd: RawFd,
}

#[allow(dead_code)]
impl Doorbell {
    /// Creates a new non-blocking CLOEXEC eventfd.
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: eventfd syscall with valid CLOEXEC and NONBLOCK flags
        let fd = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { fd })
    }

    /// Signals the doorbell by writing an 8-byte integer.
    pub(crate) fn ring(&self) {
        let val: u64 = 1;
        // SAFETY: self.fd is a valid open file descriptor; val is a valid 8-byte buffer.
        let res = unsafe {
            libc::write(
                self.fd,
                std::ptr::addr_of!(val).cast::<libc::c_void>(),
                std::mem::size_of::<u64>(),
            )
        };
        // EAGAIN / EWOULDBLOCK means counter is already non-zero and loop will wake up anyway.
        let _ = res;
    }

    /// Drains the doorbell counter upon loop wakeup.
    pub(crate) fn drain(&self) {
        let mut val: u64 = 0;
        // SAFETY: self.fd is a valid open file descriptor; val is a valid 8-byte mutable buffer.
        let res = unsafe {
            libc::read(
                self.fd,
                std::ptr::addr_of_mut!(val).cast::<libc::c_void>(),
                std::mem::size_of::<u64>(),
            )
        };
        // EAGAIN means already drained or 0, which is normal.
        let _ = res;
    }

    /// Borrow the underlying raw file descriptor.
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl AsRawFd for Doorbell {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for Doorbell {
    fn drop(&mut self) {
        // SAFETY: self.fd is an owned open file descriptor
        unsafe {
            libc::close(self.fd);
        }
    }
}
