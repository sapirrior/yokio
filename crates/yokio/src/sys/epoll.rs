//! Linux epoll selector implementation.

#![allow(unsafe_code)]

use crate::sys::events::Events;
use std::io;
use std::os::fd::{AsRawFd, RawFd};

/// OS event selector backed by Linux epoll.
pub(crate) struct Selector {
    epfd: RawFd,
}

#[allow(dead_code)]
impl Selector {
    /// Creates a new epoll selector instance with `EPOLL_CLOEXEC`.
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: epoll_create1 with valid flags
        let epfd = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        if epfd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { epfd })
    }

    /// Registers a file descriptor with interest flags and a token.
    pub(crate) fn register(&self, fd: RawFd, token: u64, interests: u32) -> io::Result<()> {
        let mut ev = libc::epoll_event {
            events: interests,
            u64: token,
        };
        // SAFETY: self.epfd is valid, fd is valid, ev is a valid local epoll_event
        let res = unsafe {
            libc::epoll_ctl(
                self.epfd,
                libc::EPOLL_CTL_ADD,
                fd,
                std::ptr::addr_of_mut!(ev),
            )
        };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Modifies registered interests for a file descriptor.
    pub(crate) fn reregister(&self, fd: RawFd, token: u64, interests: u32) -> io::Result<()> {
        let mut ev = libc::epoll_event {
            events: interests,
            u64: token,
        };
        // SAFETY: self.epfd is valid, fd was previously registered, ev is valid
        let res = unsafe {
            libc::epoll_ctl(
                self.epfd,
                libc::EPOLL_CTL_MOD,
                fd,
                std::ptr::addr_of_mut!(ev),
            )
        };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Deregisters a file descriptor from the selector.
    pub(crate) fn deregister(&self, fd: RawFd) -> io::Result<()> {
        // SAFETY: self.epfd is valid, null pointer is accepted for event in Linux >= 2.6.9
        let res =
            unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, std::ptr::null_mut()) };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Polls the kernel for readiness events, blocking up to `timeout_ms`.
    pub(crate) fn poll(&self, events: &mut Events, timeout_ms: Option<i32>) -> io::Result<()> {
        let timeout = timeout_ms.unwrap_or(-1);
        let cap = events.capacity();
        let maxevents = i32::try_from(cap).unwrap_or(i32::MAX);

        // SAFETY: events.raw has capacity of at least cap, epoll_wait writes at most maxevents
        let nfds =
            unsafe { libc::epoll_wait(self.epfd, events.raw.as_mut_ptr(), maxevents, timeout) };

        if nfds < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                events.len = 0;
                return Ok(());
            }
            return Err(err);
        }

        events.len = usize::try_from(nfds).unwrap_or(0);
        Ok(())
    }
}

impl AsRawFd for Selector {
    fn as_raw_fd(&self) -> RawFd {
        self.epfd
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        // SAFETY: self.epfd is an owned open file descriptor
        unsafe {
            libc::close(self.epfd);
        }
    }
}
