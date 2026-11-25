//! Cross-thread shared runtime state and task injection queue.

use crate::error::{Error, Result};
use crate::sys::Doorbell;
use std::sync::atomic::{fence, AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::sync::Mutex;

/// Messages passed across threads to the event loop.
pub(crate) enum Msg {
    /// A closure submitted from off-thread to run on the event loop.
    Run(Box<dyn FnOnce() + Send + 'static>),
}

pub(crate) const STATE_CREATED: u8 = 0;
pub(crate) const STATE_TERMINATED: u8 = 3;

/// Thread-safe state shared between the event loop and external handles.
pub(crate) struct Shared {
    pub(crate) injector: Mutex<Vec<Msg>>,
    pub(crate) doorbell: Doorbell,
    pub(crate) sleeping: AtomicBool,
    pub(crate) ringing: AtomicBool,
    pub(crate) stop: AtomicBool,
    pub(crate) external_refs: AtomicUsize,
    pub(crate) state: AtomicU8,
}

impl Shared {
    /// Creates a new `Shared` instance wrapping the given doorbell.
    pub(crate) fn new(doorbell: Doorbell) -> Self {
        Self {
            injector: Mutex::new(Vec::new()),
            doorbell,
            sleeping: AtomicBool::new(false),
            ringing: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            external_refs: AtomicUsize::new(0),
            state: AtomicU8::new(STATE_CREATED),
        }
    }

    /// Enqueues a message into the injector queue and rings the doorbell if the loop is sleeping.
    pub(crate) fn post(&self, msg: Msg) -> Result<()> {
        if self.state.load(Ordering::Acquire) == STATE_TERMINATED {
            return Err(Error::Shutdown);
        }

        {
            let mut guard = self.injector.lock().unwrap();
            guard.push(msg);
        }

        // Dekker pair fence: ensures consumer store(sleeping) and producer push(msg) are strictly ordered.
        fence(Ordering::SeqCst);

        // Ring the doorbell only if loop is sleeping and doorbell isn't already ringing
        if self.sleeping.load(Ordering::SeqCst) && !self.ringing.swap(true, Ordering::AcqRel) {
            self.doorbell.ring();
        }

        Ok(())
    }

    /// Swaps the injector contents into a thread-local inbox vector in O(1).
    pub(crate) fn drain_inbox(&self, inbox: &mut Vec<Msg>) {
        let mut guard = self.injector.lock().unwrap();
        if !guard.is_empty() {
            std::mem::swap(&mut *guard, inbox);
        }
    }

    /// Checks if the injector currently has pending messages without draining.
    pub(crate) fn has_messages(&self) -> bool {
        !self.injector.lock().unwrap().is_empty()
    }

    /// Requests the runtime to stop.
    pub(crate) fn request_stop(&self) {
        self.stop.store(true, Ordering::Release);
        fence(Ordering::SeqCst);
        if self.sleeping.load(Ordering::SeqCst) && !self.ringing.swap(true, Ordering::AcqRel) {
            self.doorbell.ring();
        }
    }
}
