//! Cross-platform I/O descriptors and traits.

#[cfg(unix)]
pub(crate) type RawFd = std::os::fd::RawFd;

#[cfg(unix)]
pub use std::os::fd::AsRawFd;

#[cfg(windows)]
pub(crate) type RawFd = std::os::windows::io::RawSocket;

/// Cross-platform abstraction for extracting a raw descriptor/socket.
#[cfg(windows)]
pub trait AsRawFd {
    /// Extracts the raw socket/file descriptor.
    fn as_raw_fd(&self) -> RawFd;
}

#[cfg(windows)]
impl<T: std::os::windows::io::AsRawSocket> AsRawFd for T {
    fn as_raw_fd(&self) -> RawFd {
        self.as_raw_socket()
    }
}
