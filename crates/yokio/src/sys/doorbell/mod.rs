//! Cross-thread wake primitive (Doorbell).

#[cfg(target_os = "linux")]
mod eventfd;

#[cfg(target_os = "linux")]
pub(crate) use eventfd::Doorbell;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
mod pipe;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
pub(crate) use pipe::Doorbell;

#[cfg(target_os = "windows")]
mod socket;

#[cfg(target_os = "windows")]
pub(crate) use socket::{init_winsock, Doorbell};
