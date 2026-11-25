//! Reusable event buffer for OS readiness events.

#![allow(unsafe_code)]

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
use crate::sys::os_flags;

#[cfg(target_os = "windows")]
#[derive(Copy, Clone, Default)]
pub(crate) struct WinEvent {
    pub(crate) token: u64,
    pub(crate) events: u32,
}

/// Reusable buffer containing OS readiness events from poll.
pub(crate) struct Events {
    #[cfg(target_os = "linux")]
    pub(crate) raw: Vec<libc::epoll_event>,
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    pub(crate) raw: Vec<libc::kevent>,
    #[cfg(target_os = "windows")]
    pub(crate) raw: Vec<WinEvent>,
    pub(crate) len: usize,
}

#[allow(dead_code)]
impl Events {
    /// Allocates an event buffer with the given capacity.
    pub(crate) fn with_capacity(cap: usize) -> Self {
        #[cfg(target_os = "linux")]
        {
            let mut raw = Vec::with_capacity(cap);
            raw.resize(cap, libc::epoll_event { events: 0, u64: 0 });
            Self { raw, len: 0 }
        }
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        {
            let mut raw = Vec::with_capacity(cap);
            let zero = unsafe { std::mem::zeroed::<libc::kevent>() };
            raw.resize(cap, zero);
            Self { raw, len: 0 }
        }
        #[cfg(target_os = "windows")]
        {
            let mut raw = Vec::with_capacity(cap);
            raw.resize(cap, WinEvent::default());
            Self { raw, len: 0 }
        }
    }

    /// Total event capacity of the buffer.
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        self.raw.len()
    }

    /// Whether there are no ready events.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Number of ready events captured in the latest poll.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Resets the event count for the next iteration.
    #[inline]
    pub(crate) fn clear(&mut self) {
        self.len = 0;
    }

    /// Iterates through ready `(token, event_flags)` pairs.
    #[inline]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u64, u32)> + '_ {
        #[cfg(target_os = "linux")]
        {
            self.raw[..self.len].iter().map(|ev| (ev.u64, ev.events))
        }
        #[cfg(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "freebsd",
            target_os = "openbsd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ))]
        {
            self.raw[..self.len].iter().map(|ev| {
                let token = ev.udata as usize as u64;
                let mut flags = 0u32;
                if ev.filter == libc::EVFILT_READ {
                    flags |= os_flags::READABLE;
                }
                if ev.filter == libc::EVFILT_WRITE {
                    flags |= os_flags::WRITABLE;
                }
                if (ev.flags & libc::EV_ERROR) != 0 {
                    flags |= os_flags::ERROR;
                }
                if (ev.flags & libc::EV_EOF) != 0 {
                    flags |= os_flags::HUP | os_flags::RDHUP;
                }
                (token, flags)
            })
        }
        #[cfg(target_os = "windows")]
        {
            self.raw[..self.len].iter().map(|ev| (ev.token, ev.events))
        }
    }
}
