//! Readiness interest and event notification bitflags.

use std::ops::{BitOr, BitOrAssign};

/// I/O readiness interest to register with the event loop.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Interest(u8);

impl Interest {
    /// Interest in read readiness (data available to read, connection accepted, etc.).
    pub const READABLE: Self = Self(1);
    /// Interest in write readiness (send buffer has capacity).
    pub const WRITABLE: Self = Self(2);
    /// Interest in both read and write readiness.
    pub const BOTH: Self = Self(3);

    /// Whether this includes read interest.
    #[inline]
    pub const fn is_readable(self) -> bool {
        (self.0 & Self::READABLE.0) != 0
    }

    /// Whether this includes write interest.
    #[inline]
    pub const fn is_writable(self) -> bool {
        (self.0 & Self::WRITABLE.0) != 0
    }

    /// Converts interest and trigger mode into Linux epoll event flags.
    #[cfg(target_os = "linux")]
    pub(crate) fn to_epoll(self, mode: Mode) -> u32 {
        let mut flags = 0u32;
        if self.is_readable() {
            #[allow(clippy::cast_sign_loss)]
            {
                flags |= (libc::EPOLLIN | libc::EPOLLRDHUP | libc::EPOLLPRI) as u32;
            }
        }
        if self.is_writable() {
            #[allow(clippy::cast_sign_loss)]
            {
                flags |= libc::EPOLLOUT as u32;
            }
        }
        match mode {
            Mode::Level => {}
            Mode::Edge => {
                #[allow(clippy::cast_sign_loss)]
                {
                    flags |= libc::EPOLLET as u32;
                }
            }
            Mode::Oneshot => {
                #[allow(clippy::cast_sign_loss)]
                {
                    flags |= libc::EPOLLONESHOT as u32;
                }
            }
        }
        flags
    }
}

impl BitOr for Interest {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Interest {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// Operating trigger mode for I/O readiness watchers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    /// Level-triggered (default): refires every tick as long as source remains ready.
    #[default]
    Level,
    /// Edge-triggered: fires only when new readiness transitions occur.
    Edge,
    /// Oneshot: fires once upon readiness, then automatically disarms until rearmed.
    Oneshot,
}

/// Readiness notification flags delivered to a watcher callback.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Ready(pub(crate) u32);

impl Ready {
    /// Whether the source has data available to read.
    #[inline]
    pub fn is_readable(self) -> bool {
        #[cfg(target_os = "linux")]
        {
            #[allow(clippy::cast_sign_loss)]
            let mask = (libc::EPOLLIN | libc::EPOLLPRI | libc::EPOLLRDHUP) as u32;
            (self.0 & mask) != 0
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Whether the source can accept writes without blocking.
    #[inline]
    pub fn is_writable(self) -> bool {
        #[cfg(target_os = "linux")]
        {
            #[allow(clippy::cast_sign_loss)]
            let mask = libc::EPOLLOUT as u32;
            (self.0 & mask) != 0
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Whether the source encountered an error condition.
    #[inline]
    pub fn is_error(self) -> bool {
        #[cfg(target_os = "linux")]
        {
            #[allow(clippy::cast_sign_loss)]
            let mask = libc::EPOLLERR as u32;
            (self.0 & mask) != 0
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Whether the source was hung up or remote end disconnected.
    #[inline]
    pub fn is_hup(self) -> bool {
        #[cfg(target_os = "linux")]
        {
            #[allow(clippy::cast_sign_loss)]
            let mask = (libc::EPOLLHUP | libc::EPOLLRDHUP) as u32;
            (self.0 & mask) != 0
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Whether the read side of the connection is closed.
    #[inline]
    pub fn is_read_closed(self) -> bool {
        #[cfg(target_os = "linux")]
        {
            #[allow(clippy::cast_sign_loss)]
            let mask = libc::EPOLLRDHUP as u32;
            (self.0 & mask) != 0
        }
        #[cfg(not(target_os = "linux"))]
        {
            false
        }
    }

    /// Whether the write side is closed or disconnected.
    #[inline]
    pub fn is_write_closed(self) -> bool {
        self.is_hup()
    }
}
