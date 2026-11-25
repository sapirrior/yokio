//! Cross-thread wake primitive (Doorbell).

#[cfg(target_os = "linux")]
mod eventfd;

#[cfg(target_os = "linux")]
pub(crate) use eventfd::Doorbell;

#[cfg(target_os = "windows")]
mod socket;

#[cfg(target_os = "windows")]
pub(crate) use socket::{init_winsock, Doorbell};
