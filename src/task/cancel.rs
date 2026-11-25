//! Handles for managing and cancelling scheduled tasks and timers.

use crate::key::Key;
use crate::runtime::context::with_core;
use std::time::Duration;

/// Trait implemented by all cancellable execution handles.
pub trait Cancel {
    /// Cancels execution. Returns `true` if cancelled before running.
    fn cancel(&self) -> bool;
}

/// A lightweight, Copy, loop-affine handle to a scheduled task.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub(crate) key: Key,
}

impl Task {
    /// Cancels the task if it has not yet executed.
    pub fn cancel(&self) -> bool {
        with_core(|core| core.cancel_task(self.key)).unwrap_or(false)
    }

    /// Whether the task is still queued and awaiting execution.
    pub fn is_pending(&self) -> bool {
        with_core(|core| core.is_task_pending(self.key)).unwrap_or(false)
    }

    /// Wraps the handle in an RAII guard that automatically cancels on drop.
    pub fn guard(self) -> CancelOnDrop<Self> {
        CancelOnDrop::new(self)
    }
}

impl Cancel for Task {
    fn cancel(&self) -> bool {
        self.cancel()
    }
}

/// A lightweight, Copy, loop-affine handle to a scheduled timer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timer {
    pub(crate) key: Key,
}

impl Timer {
    /// Cancels the timer. Returns `true` if cancelled before firing.
    pub fn cancel(&self) -> bool {
        with_core(|core| core.cancel_timer(self.key)).unwrap_or(false)
    }

    /// Whether the timer is currently active and armed.
    pub fn is_active(&self) -> bool {
        with_core(|core| core.is_timer_active(self.key)).unwrap_or(false)
    }

    /// Resets the timer deadline to fire after `delay` from now.
    pub fn reset(&self, delay: Duration) -> bool {
        with_core(|core| core.reset_timer(self.key, delay)).unwrap_or(false)
    }

    /// Unreferences the timer so it does not keep the event loop alive.
    pub fn unref(&self) {
        let _ = with_core(|core| core.unref_timer(self.key));
    }

    /// Wraps the timer in an RAII guard that automatically cancels on drop.
    pub fn guard(self) -> CancelOnDrop<Self> {
        CancelOnDrop::new(self)
    }
}

impl Cancel for Timer {
    fn cancel(&self) -> bool {
        self.cancel()
    }
}

/// RAII guard that automatically cancels the wrapped handle when dropped.
pub struct CancelOnDrop<T: Cancel> {
    target: Option<T>,
}

impl<T: Cancel> CancelOnDrop<T> {
    /// Creates a new cancellation guard wrapping `target`.
    pub fn new(target: T) -> Self {
        Self {
            target: Some(target),
        }
    }

    /// Disarms the guard, consuming it without cancelling the handle.
    pub fn disarm(mut self) -> T {
        self.target
            .take()
            .expect("invariant: CancelOnDrop target present")
    }
}

impl<T: Cancel> Drop for CancelOnDrop<T> {
    fn drop(&mut self) {
        if let Some(target) = self.target.take() {
            target.cancel();
        }
    }
}
