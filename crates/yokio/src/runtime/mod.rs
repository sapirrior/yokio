//! Runtime engine driving the callback-native event loop.

pub(crate) mod context;
pub(crate) mod core;
pub(crate) mod handle;
pub(crate) mod shared;

use crate::blocking::Blocking;
use crate::error::Result;
pub use context::EnterGuard;
pub use core::TickStatus;
pub use handle::{Handle, KeepAlive};

use crate::io::interest::{Interest, Mode, Ready};
use crate::io::watch::{AsSource, Watch, WatchBuilder};
use crate::runtime::core::LoopCore;
use crate::task::callback::{InlineFnMut, InlineFnOnce};
use crate::task::cancel::{Task, Timer};
use crate::time::clock::Clock;
use crate::time::wheel::{Missed, TimerCb};
use std::rc::Rc;
use std::time::Duration;

/// An embeddable single-threaded Yokio runtime instance.
pub struct Runtime {
    core: Rc<LoopCore>,
}

impl Runtime {
    /// Creates a new `Runtime` instance with default capacity.
    pub fn new() -> Result<Self> {
        let core = LoopCore::new(1024)?;
        Ok(Self { core })
    }

    /// Obtains an RAII guard making this runtime active on the current thread.
    pub fn enter(&self) -> EnterGuard {
        EnterGuard::new(Rc::clone(&self.core))
    }

    /// Obtains a thread-safe `Handle` to this runtime.
    pub fn handle(&self) -> Handle {
        self.core.handle()
    }

    /// Schedules a one-shot task on the runtime.
    pub fn task<F: FnOnce() + 'static>(&self, f: F) -> Task {
        self.core.spawn_task(InlineFnOnce::from_fn(f))
    }

    /// Schedules a one-shot timer firing after `delay`.
    pub fn after<F: FnOnce() + 'static>(&self, delay: Duration, f: F) -> Timer {
        let add_ticks = Clock::duration_to_ticks_ceil(delay);
        let when = self.core.now_tick().saturating_add(add_ticks);
        self.core.spawn_timer(
            when,
            0,
            Missed::Skip,
            1,
            TimerCb::Once(Some(InlineFnOnce::from_fn(f))),
        )
    }

    /// Schedules a recurring timer firing every `period`.
    pub fn every<F: FnMut() + 'static>(&self, period: Duration, f: F) -> Timer {
        let p_ticks = Clock::duration_to_ticks_ceil(period);
        let when = self.core.now_tick().saturating_add(p_ticks);
        self.core.spawn_timer(
            when,
            p_ticks,
            Missed::Skip,
            u32::MAX,
            TimerCb::Repeat(Some(InlineFnMut::from_fn(f))),
        )
    }

    /// Watches an I/O source for readiness events.
    pub fn watch<S: AsSource, F: FnMut(Ready) + 'static>(
        &self,
        src: &S,
        interest: Interest,
        f: F,
    ) -> Result<Watch> {
        self.core
            .register_watch(src.raw_source(), interest, Mode::Level, true, f)
    }

    /// Obtains a `WatchBuilder` to configure advanced options on an I/O watcher.
    pub fn watcher<'a, S: AsSource>(&self, src: &'a S) -> WatchBuilder<'a, S> {
        WatchBuilder::new(src)
    }

    /// Submits a blocking job to the worker pool and invokes `done` on the event loop.
    pub fn blocking<J, R, D>(&self, job: J, done: D) -> Blocking
    where
        J: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
        D: FnOnce(R) + 'static,
    {
        self.core.spawn_blocking(job, done)
    }

    /// Submits a blocking job without a completion callback.
    pub fn blocking_detached<J: FnOnce() + Send + 'static>(&self, job: J) -> Blocking {
        self.core.spawn_blocking_detached(job)
    }

    /// Executes a single event loop iteration.
    pub fn tick(&self, max_wait: Option<Duration>) -> Result<TickStatus> {
        let _guard = self.enter();
        self.core.tick(max_wait)
    }

    /// Drives the runtime event loop until idle or stopped.
    pub fn run(&self) -> Result<()> {
        let _guard = self.enter();
        while let TickStatus::Progress = self.core.tick(None)? {}
        Ok(())
    }

    /// Runs a root closure inside the runtime context, driving the loop to completion.
    pub fn run_with<T: 'static, F: FnOnce() -> T + 'static>(&self, main: F) -> Result<T> {
        let _guard = self.enter();
        let result_cell = std::rc::Rc::new(std::cell::Cell::new(None));
        let rc_clone = std::rc::Rc::clone(&result_cell);

        self.core.spawn_task(InlineFnOnce::from_fn(move || {
            let res = main();
            rc_clone.set(Some(res));
        }));

        self.run()?;

        Ok(result_cell.take().expect("main closure did not complete"))
    }
}
