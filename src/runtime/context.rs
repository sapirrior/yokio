//! Thread-local execution context tracking the active event loop.

use crate::error::{Error, Result};
use crate::runtime::core::LoopCore;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static CURRENT: RefCell<Option<Rc<LoopCore>>> = const { RefCell::new(None) };
}

/// RAII guard setting the thread-local runtime pointer and restoring the previous one on drop.
pub struct EnterGuard {
    prev: Option<Rc<LoopCore>>,
}

impl EnterGuard {
    pub(crate) fn new(core: Rc<LoopCore>) -> Self {
        let prev = CURRENT.with(|cell| cell.replace(Some(core)));
        Self { prev }
    }
}

impl Drop for EnterGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        CURRENT.with(|cell| cell.replace(prev));
    }
}

/// Executes a closure with a borrowed reference to the current thread's `LoopCore`.
pub(crate) fn with_core<R>(f: impl FnOnce(&LoopCore) -> R) -> Result<R> {
    let core = CURRENT.with(|cell| cell.borrow().clone());
    let Some(core) = core else {
        return Err(Error::NotOnLoopThread);
    };
    Ok(f(&core))
}
