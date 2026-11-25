//! Off-thread elastic blocking execution pool.

pub(crate) mod pool;

pub(crate) use pool::BlockingPool;

use crate::key::Key;
use crate::runtime::context::with_core;
use crate::task::cancel::{Cancel, CancelOnDrop};

/// A lightweight, Copy, loop-affine handle to an in-flight blocking operation.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Blocking {
    pub(crate) key: Key,
}

impl Blocking {
    /// Cancels the blocking result callback if it has not yet run.
    pub fn cancel(&self) -> bool {
        with_core(|core| core.cancel_blocking(self.key)).unwrap_or(false)
    }

    /// Whether the blocking operation is still in flight or awaiting its completion callback.
    pub fn is_pending(&self) -> bool {
        with_core(|core| core.is_blocking_pending(self.key)).unwrap_or(false)
    }

    /// Wraps the handle in an RAII guard that automatically cancels on drop.
    pub fn guard(self) -> CancelOnDrop<Self> {
        CancelOnDrop::new(self)
    }
}

impl Cancel for Blocking {
    fn cancel(&self) -> bool {
        self.cancel()
    }
}
