//! Windows loopback UDP doorbell for cross-thread wakeups.

#![allow(
    unsafe_code,
    clippy::cast_lossless,
    clippy::unnecessary_cast,
    clippy::cast_possible_wrap
)]

use crate::sys::types::RawFd;
use std::io;
use std::sync::Once;
use windows_sys::Win32::Networking::WinSock::{
    bind, closesocket, connect, getsockname, ioctlsocket, recv, send, socket, WSAStartup, AF_INET,
    FIONBIO, INVALID_SOCKET, SOCKADDR, SOCKADDR_IN, SOCKET, SOCK_DGRAM, WSADATA,
};

static WINSOCK_INIT: Once = Once::new();

pub(crate) fn init_winsock() {
    WINSOCK_INIT.call_once(|| {
        let mut data: WSADATA = unsafe { std::mem::zeroed() };
        unsafe {
            WSAStartup(0x0202, &mut data);
        }
    });
}

/// Cross-thread doorbell primitive signaling sleeping event loops via local loopback UDP socket.
pub(crate) struct Doorbell {
    sock: SOCKET,
}

impl Doorbell {
    /// Creates a new non-blocking loopback UDP socket.
    pub(crate) fn new() -> io::Result<Self> {
        init_winsock();

        // SAFETY: socket creation with standard AF_INET and SOCK_DGRAM
        let sock = unsafe { socket(AF_INET as i32, SOCK_DGRAM as i32, 0) };
        if sock == INVALID_SOCKET {
            return Err(io::Error::last_os_error());
        }

        // Set non-blocking mode
        let mut nonblocking: u32 = 1;
        let res = unsafe { ioctlsocket(sock, FIONBIO, &mut nonblocking) };
        if res != 0 {
            unsafe {
                closesocket(sock);
            }
            return Err(io::Error::last_os_error());
        }

        // Bind to 127.0.0.1:0
        let mut addr: SOCKADDR_IN = unsafe { std::mem::zeroed() };
        addr.sin_family = AF_INET;
        addr.sin_addr.S_un.S_addr = u32::from_ne_bytes([127, 0, 0, 1]);
        addr.sin_port = 0;

        let res = unsafe {
            bind(
                sock,
                std::ptr::addr_of!(addr).cast::<SOCKADDR>(),
                std::mem::size_of::<SOCKADDR_IN>() as i32,
            )
        };
        if res != 0 {
            unsafe {
                closesocket(sock);
            }
            return Err(io::Error::last_os_error());
        }

        // Get assigned port
        let mut namelen = std::mem::size_of::<SOCKADDR_IN>() as i32;
        let res = unsafe {
            getsockname(
                sock,
                std::ptr::addr_of_mut!(addr).cast::<SOCKADDR>(),
                &mut namelen,
            )
        };
        if res != 0 {
            unsafe {
                closesocket(sock);
            }
            return Err(io::Error::last_os_error());
        }

        // Connect to itself so send() without address works
        let res = unsafe {
            connect(
                sock,
                std::ptr::addr_of!(addr).cast::<SOCKADDR>(),
                std::mem::size_of::<SOCKADDR_IN>() as i32,
            )
        };
        if res != 0 {
            unsafe {
                closesocket(sock);
            }
            return Err(io::Error::last_os_error());
        }

        Ok(Self { sock })
    }

    /// Signals the doorbell by sending a single byte.
    pub(crate) fn ring(&self) {
        let byte: u8 = 1;
        // SAFETY: self.sock is valid, buffer is valid byte pointer
        let _ = unsafe { send(self.sock, std::ptr::addr_of!(byte).cast::<u8>(), 1, 0) };
    }

    /// Drains the doorbell by reading all pending bytes.
    pub(crate) fn drain(&self) {
        let mut buf = [0u8; 64];
        loop {
            // SAFETY: self.sock is non-blocking, buf is valid
            let n = unsafe {
                recv(
                    self.sock,
                    buf.as_mut_ptr().cast::<u8>(),
                    buf.len() as i32,
                    0,
                )
            };
            if n <= 0 {
                break;
            }
        }
    }

    /// Borrow the underlying raw socket descriptor.
    pub(crate) fn raw_fd(&self) -> RawFd {
        self.sock as RawFd
    }
}

impl Drop for Doorbell {
    fn drop(&mut self) {
        // SAFETY: self.sock is an owned open socket
        unsafe {
            closesocket(self.sock);
        }
    }
}
