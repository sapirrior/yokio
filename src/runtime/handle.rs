//! External thread-safe runtime handle and `KeepAlive` tokens.

use crate::error::Result;
use crate::runtime::context::with_core;
use crate::runtime::shared::{Msg, Shared};
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Cloneable, Send + Sync handle to inject work or control an event loop from any thread.
#[derive(Clone)]
pub struct Handle {
    pub(crate) shared: Arc<Shared>,
}

impl Handle {
    /// Obtains a `Handle` to the current thread's runtime.
    pub fn current() -> Self {
        Self::try_current().expect("yokio: called outside of a Yokio runtime. Use #[yokio::main], Runtime::run_with, or Runtime::enter().")
    }

    /// Attempts to obtain a `Handle` to the current thread's runtime.
    pub fn try_current() -> Option<Self> {
        with_core(super::core::LoopCore::handle).ok()
    }

    /// Submits a closure from any thread to be executed on the event loop.
    pub fn post<F: FnOnce() + Send + 'static>(&self, f: F) -> Result<()> {
        self.shared.post(Msg::Run(Box::new(f)))
    }

    /// Requests the runtime to stop after completing the current tick.
    pub fn stop(&self) {
        self.shared.request_stop();
    }

    /// Obtains an RAII token that keeps the runtime running even if there is no other work.
    pub fn keep_alive(&self) -> KeepAlive {
        self.shared.external_refs.fetch_add(1, Ordering::SeqCst);
        KeepAlive {
            shared: Arc::clone(&self.shared),
        }
    }
}

/// RAII token that prevents the runtime event loop from exiting while held.
pub struct KeepAlive {
    shared: Arc<Shared>,
}

impl Drop for KeepAlive {
    fn drop(&mut self) {
        self.shared.external_refs.fetch_sub(1, Ordering::SeqCst);
    }
}
