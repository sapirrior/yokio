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
pub(crate) mod sys;
pub(crate) mod task;
pub mod time;

pub use error::{CallbackKind, Error, JobPanic, PanicInfo, Result};
pub use time::Missed;
