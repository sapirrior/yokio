//! I/O watcher handle, builder, and source traits.

use crate::error::Result;
use crate::io::interest::{Interest, Mode, Ready};
use crate::key::Key;
use crate::runtime::context::with_core;
use crate::sys::types::{AsRawFd, RawFd};
use crate::task::cancel::{Cancel, CancelOnDrop};

/// Trait implemented by pollable I/O resources (sockets, pipes, stdin, etc.).
pub trait AsSource {
    /// Obtains the underlying raw file descriptor to register with the OS selector.
    fn raw_source(&self) -> RawFd;
}

impl<T: AsRawFd> AsSource for T {
    fn raw_source(&self) -> RawFd {
        self.as_raw_fd()
    }
}

/// A lightweight, Copy, loop-affine handle to an active I/O readiness watcher.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Watch {
    pub(crate) key: Key,
}

impl Watch {
    /// Cancels the watcher, deregistering its source from the OS selector.
    pub fn cancel(&self) -> bool {
        with_core(|core| core.cancel_watch(self.key)).unwrap_or(false)
    }

    /// Modifies the registered interest flags for this watcher.
    pub fn modify(&self, interest: Interest) -> Result<()> {
        with_core(|core| core.modify_watch(self.key, interest))?
    }

    /// Rearms an inactive `Mode::Oneshot` watcher.
    pub fn rearm(&self) -> Result<()> {
        with_core(|core| core.rearm_watch(self.key))?
    }

    /// Unreferences the watcher so it does not keep the event loop alive.
    pub fn unref(&self) {
        let _ = with_core(|core| core.unref_watch(self.key));
    }

    /// Wraps the handle in an RAII guard that automatically cancels on drop.
    pub fn guard(self) -> CancelOnDrop<Self> {
        CancelOnDrop::new(self)
    }
}

impl Cancel for Watch {
    fn cancel(&self) -> bool {
        self.cancel()
    }
}

/// Builder configuring trigger mode, interests, and liveness for an I/O watcher.
#[must_use = "watchers do nothing until `.start(...)` is called"]
pub struct WatchBuilder<'a, S: AsSource> {
    source: &'a S,
    interest: Interest,
    mode: Mode,
    refd: bool,
}

impl<'a, S: AsSource> WatchBuilder<'a, S> {
    /// Creates a new `WatchBuilder` for the given I/O source.
    pub fn new(source: &'a S) -> Self {
        Self {
            source,
            interest: Interest::READABLE,
            mode: Mode::Level,
            refd: true,
        }
    }

    /// Sets the interest to watch for (readable, writable, or both).
    pub fn interest(mut self, interest: Interest) -> Self {
        self.interest = interest;
        self
    }

    /// Sets the trigger mode (Level, Edge, or Oneshot).
    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// Configures the watcher to not keep the event loop alive.
    pub fn unref(mut self) -> Self {
        self.refd = false;
        self
    }

    /// Registers the watcher with the current runtime and starts listening.
    pub fn start<F: FnMut(Ready) + 'static>(self, f: F) -> Result<Watch> {
        with_core(|core| {
            core.register_watch(
                self.source.raw_source(),
                self.interest,
                self.mode,
                self.refd,
                f,
            )
        })?
    }
}
