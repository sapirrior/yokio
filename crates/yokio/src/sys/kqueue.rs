//! macOS and BSD kqueue selector implementation.

#![allow(
    unsafe_code,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::similar_names,
    clippy::unnecessary_wraps,
    clippy::cast_lossless
)]

use crate::sys::events::Events;
use crate::sys::flags::os_flags;
use crate::sys::types::RawFd;
use std::io;

/// OS event selector backed by kqueue.
pub(crate) struct Selector {
    kq: RawFd,
}

impl Selector {
    /// Creates a new kqueue instance with close-on-exec.
    pub(crate) fn new() -> io::Result<Self> {
        // SAFETY: kqueue syscall
        let kq = unsafe { libc::kqueue() };
        if kq < 0 {
            return Err(io::Error::last_os_error());
        }

        // Set close-on-exec
        // SAFETY: kq is valid open file descriptor
        unsafe {
            let flags = libc::fcntl(kq, libc::F_GETFD, 0);
            if flags >= 0 {
                let _ = libc::fcntl(kq, libc::F_SETFD, flags | libc::FD_CLOEXEC);
            }
        }

        Ok(Self { kq })
    }

    /// Registers a file descriptor with interest flags and a token.
    pub(crate) fn register(&self, fd: RawFd, token: u64, interests: u32) -> io::Result<()> {
        let mut changes = Vec::with_capacity(2);

        let mut flags = libc::EV_ADD | libc::EV_ENABLE;
        if (interests & os_flags::EDGE) != 0 {
            flags |= libc::EV_CLEAR;
        }
        if (interests & os_flags::ONESHOT) != 0 {
            flags |= libc::EV_ONESHOT;
        }

        if (interests & os_flags::READABLE) != 0 {
            changes.push(libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_READ,
                flags,
                fflags: 0,
                data: 0,
                udata: token as usize as *mut libc::c_void,
            });
        }

        if (interests & os_flags::WRITABLE) != 0 {
            changes.push(libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_WRITE,
                flags,
                fflags: 0,
                data: 0,
                udata: token as usize as *mut libc::c_void,
            });
        }

        if !changes.is_empty() {
            // SAFETY: self.kq is valid, changes has valid pointer and length
            let res = unsafe {
                libc::kevent(
                    self.kq,
                    changes.as_ptr(),
                    changes.len() as i32,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null(),
                )
            };
            if res < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    /// Modifies registered interests for a file descriptor.
    pub(crate) fn reregister(&self, fd: RawFd, token: u64, interests: u32) -> io::Result<()> {
        self.deregister(fd)?;
        self.register(fd, token, interests)
    }

    /// Deregisters a file descriptor from the selector.
    pub(crate) fn deregister(&self, fd: RawFd) -> io::Result<()> {
        let deletes = [
            libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_READ,
                flags: libc::EV_DELETE,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            },
            libc::kevent {
                ident: fd as usize,
                filter: libc::EVFILT_WRITE,
                flags: libc::EV_DELETE,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            },
        ];

        // SAFETY: EV_DELETE may return ENOENT if filter wasn't registered, which is safe to ignore
        unsafe {
            libc::kevent(
                self.kq,
                deletes.as_ptr(),
                deletes.len() as i32,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            );
        }

        Ok(())
    }

    /// Polls the kernel for readiness events, blocking up to `timeout_ms`.
    pub(crate) fn poll(&self, events: &mut Events, timeout_ms: Option<i32>) -> io::Result<()> {
        let timeout_spec = timeout_ms.map(|ms| {
            let sec = (ms / 1000) as libc::time_t;
            let nsec = ((ms % 1000) * 1_000_000) as libc::c_long;
            libc::timespec {
                tv_sec: sec,
                tv_nsec: nsec,
            }
        });

        let ts_ptr = match &timeout_spec {
            Some(spec) => std::ptr::addr_of!(*spec),
            None => std::ptr::null(),
        };

        let cap = events.capacity();
        let maxevents = cap as i32;

        // SAFETY: self.kq is valid, events.raw has capacity of at least cap
        let n = unsafe {
            libc::kevent(
                self.kq,
                std::ptr::null(),
                0,
                events.raw.as_mut_ptr(),
                maxevents,
                ts_ptr,
            )
        };

        if n < 0 {
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EINTR) {
                events.len = 0;
                return Ok(());
            }
            return Err(err);
        }

        events.len = n as usize;
        Ok(())
    }
}

impl Drop for Selector {
    fn drop(&mut self) {
        // SAFETY: self.kq is an owned open file descriptor
        unsafe {
            libc::close(self.kq);
        }
    }
}
