//! Internal watcher registry managing registered sources and callbacks.

use crate::io::interest::{Interest, Mode, Ready};
use crate::key::Key;
use crate::task::callback::InlineFnMut;
use crate::task::slab::Slab;
use std::os::fd::RawFd;

pub(crate) struct WatchEntry {
    pub(crate) fd: RawFd,
    pub(crate) interest: Interest,
    pub(crate) mode: Mode,
    pub(crate) refd: bool,
    pub(crate) armed: bool,
    pub(crate) cb: Option<InlineFnMut<Ready>>,
}

pub(crate) struct Registry {
    slab: Slab<WatchEntry>,
    refd_count: usize,
}

impl Registry {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            slab: Slab::with_capacity(capacity),
            refd_count: 0,
        }
    }

    pub(crate) fn insert(
        &mut self,
        fd: RawFd,
        interest: Interest,
        mode: Mode,
        refd: bool,
        cb: InlineFnMut<Ready>,
    ) -> Key {
        if refd {
            self.refd_count += 1;
        }
        self.slab.insert(WatchEntry {
            fd,
            interest,
            mode,
            refd,
            armed: true,
            cb: Some(cb),
        })
    }

    pub(crate) fn cancel(&mut self, key: Key) -> Option<RawFd> {
        let entry = self.slab.remove(key)?;
        if entry.refd {
            self.refd_count = self.refd_count.saturating_sub(1);
        }
        Some(entry.fd)
    }

    pub(crate) fn get_mut(&mut self, key: Key) -> Option<&mut WatchEntry> {
        self.slab.get_mut(key)
    }

    pub(crate) fn unref(&mut self, key: Key) {
        if let Some(entry) = self.slab.get_mut(key) {
            if entry.refd {
                entry.refd = false;
                self.refd_count = self.refd_count.saturating_sub(1);
            }
        }
    }

    pub(crate) fn has_refd_watchers(&self) -> bool {
        self.refd_count > 0
    }
}
