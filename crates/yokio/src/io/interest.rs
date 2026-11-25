//! Readiness interest and event notification bitflags.

use crate::sys::os_flags;
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

    /// Converts interest and trigger mode into platform OS event flags.
    pub(crate) fn to_os_flags(self, mode: Mode) -> u32 {
        let mut flags = 0u32;
        if self.is_readable() {
            flags |= os_flags::READABLE;
        }
        if self.is_writable() {
            flags |= os_flags::WRITABLE;
        }
        match mode {
            Mode::Level => {}
            Mode::Edge => {
                flags |= os_flags::EDGE;
            }
            Mode::Oneshot => {
                flags |= os_flags::ONESHOT;
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
        (self.0 & os_flags::READABLE) != 0
    }

    /// Whether the source can accept writes without blocking.
    #[inline]
    pub fn is_writable(self) -> bool {
        (self.0 & os_flags::WRITABLE) != 0
    }

    /// Whether the source encountered an error condition.
    #[inline]
    pub fn is_error(self) -> bool {
        (self.0 & os_flags::ERROR) != 0
    }

    /// Whether the source was hung up or remote end disconnected.
    #[inline]
    pub fn is_hup(self) -> bool {
        (self.0 & os_flags::HUP) != 0
    }

    /// Whether the read side of the connection is closed.
    #[inline]
    pub fn is_read_closed(self) -> bool {
        (self.0 & os_flags::RDHUP) != 0
    }

    /// Whether the write side is closed or disconnected.
    #[inline]
    pub fn is_write_closed(self) -> bool {
        self.is_hup()
    }
}
