//! Cross-thread wake primitive (Doorbell).

#[cfg(target_os = "linux")]
mod eventfd;

#[cfg(target_os = "linux")]
pub(crate) use eventfd::Doorbell;
