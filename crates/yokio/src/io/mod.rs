//! Asynchronous OS I/O readiness monitoring.

pub mod interest;
pub(crate) mod registry;
pub mod watch;

pub use interest::{Interest, Mode, Ready};
pub use watch::{AsSource, Watch, WatchBuilder};
