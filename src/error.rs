//! Error types and panic information for Yokio.

use std::fmt;

/// The error type for Yokio operations.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// An underlying I/O error occurred.
    Io(std::io::Error),
    /// The runtime is shutting down or has already terminated.
    Shutdown,
    /// The runtime is already executing on the current thread.
    AlreadyRunning,
    /// Attempted to call a loop-affine free function outside a Yokio runtime context.
    NotOnLoopThread,
    /// An invalid argument was provided to a builder or function.
    InvalidArgument(&'static str),
    /// A user callback panicked.
    Panic(PanicInfo),
    /// The requested operation or platform backend is unsupported.
    Unsupported(&'static str),
}

/// Convenience alias for `std::result::Result<T, yokio::Error>`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Diagnostic information about a callback panic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanicInfo {
    /// Panic payload message extracted from `catch_unwind`.
    pub message: String,
    /// Kind of callback that triggered the panic.
    pub kind: CallbackKind,
    /// Loop index on which the panic occurred.
    pub loop_index: usize,
}

/// Category of callback executing when an event or panic occurred.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CallbackKind {
    /// A one-shot scheduled task.
    Task,
    /// A timer callback.
    Timer,
    /// An I/O readiness watcher callback.
    Watch,
    /// A blocking job result callback.
    Blocking,
    /// A cross-thread trigger notification.
    Trigger,
    /// An inter-thread channel message.
    Message,
    /// A deferred drop in the graveyard.
    Drop,
    /// The runtime main entry closure.
    Main,
}

/// Information describing a panic that occurred inside an off-thread blocking job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobPanic {
    /// Panic message extracted from the thread.
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::Shutdown => write!(f, "runtime is shutting down or terminated"),
            Self::AlreadyRunning => write!(f, "runtime is already running"),
            Self::NotOnLoopThread => write!(
                f,
                "called outside of a yokio runtime. Use #[yokio::main], Runtime::run_with, or Runtime::enter()"
            ),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            Self::Panic(info) => write!(f, "callback panicked: {}", info.message),
            Self::Unsupported(msg) => write!(f, "unsupported operation: {msg}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl fmt::Display for JobPanic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "blocking job panicked: {}", self.message)
    }
}

impl std::error::Error for JobPanic {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formatting() {
        let err = Error::InvalidArgument("timeout cannot be zero");
        assert_eq!(err.to_string(), "invalid argument: timeout cannot be zero");

        let not_on_loop = Error::NotOnLoopThread;
        assert!(not_on_loop
            .to_string()
            .contains("outside of a yokio runtime"));
    }

    #[test]
    fn io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: Error = io_err.into();
        match err {
            Error::Io(_) => {}
            _ => panic!("expected Error::Io"),
        }
    }
}
