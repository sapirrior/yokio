//! Platform-neutral readiness event flags.

#[cfg(target_os = "linux")]
#[allow(clippy::cast_sign_loss)]
pub(crate) mod os_flags {
    /// Readable readiness mask.
    pub(crate) const READABLE: u32 = (libc::EPOLLIN | libc::EPOLLRDHUP | libc::EPOLLPRI) as u32;
    /// Writable readiness mask.
    pub(crate) const WRITABLE: u32 = libc::EPOLLOUT as u32;
    /// Error condition mask.
    pub(crate) const ERROR: u32 = libc::EPOLLERR as u32;
    /// Hangup condition mask.
    pub(crate) const HUP: u32 = (libc::EPOLLHUP | libc::EPOLLRDHUP) as u32;
    /// Peer closed connection mask.
    pub(crate) const RDHUP: u32 = libc::EPOLLRDHUP as u32;
    /// Edge-triggered mode mask.
    pub(crate) const EDGE: u32 = libc::EPOLLET as u32;
    /// One-shot trigger mode mask.
    pub(crate) const ONESHOT: u32 = libc::EPOLLONESHOT as u32;
    /// Doorbell registration mask.
    pub(crate) const DOORBELL: u32 = (libc::EPOLLIN | libc::EPOLLET) as u32;
}

#[cfg(target_os = "windows")]
pub(crate) mod os_flags {
    use windows_sys::Win32::Networking::WinSock::{POLLERR, POLLHUP, POLLIN, POLLOUT};

    /// Readable readiness mask (Winsock POLLIN).
    pub(crate) const READABLE: u32 = POLLIN as u32;
    /// Writable readiness mask (Winsock POLLOUT).
    pub(crate) const WRITABLE: u32 = POLLOUT as u32;
    /// Error condition mask (Winsock POLLERR).
    pub(crate) const ERROR: u32 = POLLERR as u32;
    /// Hangup condition mask (Winsock POLLHUP).
    pub(crate) const HUP: u32 = POLLHUP as u32;
    /// Peer closed connection mask.
    pub(crate) const RDHUP: u32 = POLLHUP as u32;
    /// Edge-triggered mode mask (not directly used by `WSAPoll`).
    pub(crate) const EDGE: u32 = 0x4000_0000;
    /// One-shot trigger mode mask.
    pub(crate) const ONESHOT: u32 = 0x8000_0000;
    /// Doorbell registration mask.
    pub(crate) const DOORBELL: u32 = POLLIN as u32;
}
