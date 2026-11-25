//! Reusable event buffer for OS readiness events.

#![allow(unsafe_code)]

/// Reusable buffer containing OS readiness events from poll.
pub(crate) struct Events {
    #[cfg(target_os = "linux")]
    pub(crate) raw: Vec<libc::epoll_event>,
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
        #[cfg(not(target_os = "linux"))]
        {
            Self { len: 0 }
        }
    }

    /// Total event capacity of the buffer.
    #[inline]
    pub(crate) fn capacity(&self) -> usize {
        #[cfg(target_os = "linux")]
        {
            self.raw.len()
        }
        #[cfg(not(target_os = "linux"))]
        {
            0
        }
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
    }
}
