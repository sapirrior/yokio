//! Yokio: A callback-native asynchronous execution engine for Rust.
//!
//! Yokio owns an event loop, runs callbacks (not futures) to completion,
//! fires timers, dispatches OS readiness events, coordinates threads without blocking the loop,
//! and runs blocking work on an elastic pool.

#![deny(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn, missing_docs, rust_2018_idioms)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation
)]

pub mod error;
pub(crate) mod key;
pub mod runtime;
pub(crate) mod sys;
pub mod task;
pub mod time;

pub use error::{CallbackKind, Error, JobPanic, PanicInfo, Result};
pub use runtime::{EnterGuard, Handle, KeepAlive, Runtime, TickStatus};
pub use task::{Cancel, CancelOnDrop, Task, Timer};
pub use time::Missed;

use std::time::{Duration, Instant};

/// Schedules a one-shot task to run on the current thread's event loop.
///
/// # Panics
/// Panics if called outside of a Yokio runtime thread.
pub fn task<F: FnOnce() + 'static>(f: F) -> Task {
    runtime::context::with_core(|core| core.spawn_task(task::callback::InlineFnOnce::from_fn(f)))
        .expect("yokio: called outside of a Yokio runtime. Use #[yokio::main], Runtime::run_with, or Runtime::enter().")
}

/// Schedules a one-shot timer to fire after `delay`.
///
/// # Panics
/// Panics if called outside of a Yokio runtime thread.
pub fn after<F: FnOnce() + 'static>(delay: Duration, f: F) -> Timer {
    let add_ticks = time::clock::Clock::duration_to_ticks_ceil(delay);
    runtime::context::with_core(|core| {
        let when = core.now_tick().saturating_add(add_ticks);
        core.spawn_timer(
            when,
            0,
            time::wheel::Missed::Skip,
            1,
            time::wheel::TimerCb::Once(Some(task::callback::InlineFnOnce::from_fn(f))),
        )
    })
    .expect("yokio: called outside of a Yokio runtime. Use #[yokio::main], Runtime::run_with, or Runtime::enter().")
}

/// Schedules a recurring timer to fire every `period`.
///
/// # Panics
/// Panics if called outside of a Yokio runtime thread.
pub fn every<F: FnMut() + 'static>(period: Duration, f: F) -> Timer {
    let p_ticks = time::clock::Clock::duration_to_ticks_ceil(period);
    runtime::context::with_core(|core| {
        let when = core.now_tick().saturating_add(p_ticks);
        core.spawn_timer(
            when,
            p_ticks,
            time::wheel::Missed::Skip,
            u32::MAX,
            time::wheel::TimerCb::Repeat(Some(task::callback::InlineFnMut::from_fn(f))),
        )
    })
    .expect("yokio: called outside of a Yokio runtime. Use #[yokio::main], Runtime::run_with, or Runtime::enter().")
}

/// Requests the current thread's runtime to stop after completing the current tick.
pub fn stop() {
    let _ = runtime::context::with_core(|core| core.shared.request_stop());
}

/// Returns the cached `Instant` captured at the start of the current tick.
///
/// # Panics
/// Panics if called outside of a Yokio runtime thread.
pub fn now() -> Instant {
    runtime::context::with_core(runtime::core::LoopCore::now)
        .expect("yokio: called outside of a Yokio runtime. Use #[yokio::main], Runtime::run_with, or Runtime::enter().")
}

/// Returns a clone of the thread-safe `Handle` to the current runtime.
///
/// # Panics
/// Panics if called outside of a Yokio runtime thread.
pub fn handle() -> Handle {
    Handle::current()
}
