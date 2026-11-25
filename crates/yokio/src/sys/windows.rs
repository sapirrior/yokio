//! Windows socket selector backed by `WSAPoll`.

#![allow(
    unsafe_code,
    clippy::similar_names,
    clippy::cast_sign_loss,
    clippy::unnecessary_wraps,
    clippy::unnecessary_cast
)]

use crate::sys::doorbell::init_winsock;
use crate::sys::events::{Events, WinEvent};
use crate::sys::types::RawFd;
use std::cell::RefCell;
use std::io;
use windows_sys::Win32::Networking::WinSock::{WSAPoll, POLLIN, POLLOUT, SOCKET, WSAPOLLFD};

#[derive(Clone, Copy)]
struct Registration {
    socket: SOCKET,
    token: u64,
    interests: u32,
}

pub(crate) struct Selector {
    registrations: RefCell<Vec<Registration>>,
    poll_fds: RefCell<Vec<WSAPOLLFD>>,
}

impl Selector {
    pub(crate) fn new() -> io::Result<Self> {
        init_winsock();
        Ok(Self {
            registrations: RefCell::new(Vec::new()),
            poll_fds: RefCell::new(Vec::new()),
        })
    }

    pub(crate) fn register(&self, fd: RawFd, token: u64, interests: u32) -> io::Result<()> {
        let mut regs = self.registrations.borrow_mut();
        if regs.iter().any(|r| r.socket == fd as SOCKET) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "socket already registered",
            ));
        }
        regs.push(Registration {
            socket: fd as SOCKET,
            token,
            interests,
        });
        Ok(())
    }

    pub(crate) fn reregister(&self, fd: RawFd, token: u64, interests: u32) -> io::Result<()> {
        let mut regs = self.registrations.borrow_mut();
        if let Some(r) = regs.iter_mut().find(|r| r.socket == fd as SOCKET) {
            r.token = token;
            r.interests = interests;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "socket not registered",
            ))
        }
    }

    pub(crate) fn deregister(&self, fd: RawFd) -> io::Result<()> {
        let mut regs = self.registrations.borrow_mut();
        if let Some(idx) = regs.iter().position(|r| r.socket == fd as SOCKET) {
            regs.swap_remove(idx);
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "socket not registered",
            ))
        }
    }

    pub(crate) fn poll(&self, events: &mut Events, timeout_ms: Option<i32>) -> io::Result<()> {
        let regs = self.registrations.borrow();
        if regs.is_empty() {
            if let Some(ms) = timeout_ms {
                if ms > 0 {
                    std::thread::sleep(std::time::Duration::from_millis(ms as u64));
                }
            }
            events.len = 0;
            return Ok(());
        }

        let mut pfds = self.poll_fds.borrow_mut();
        pfds.clear();
        for r in regs.iter() {
            let mut ev: i16 = 0;
            if r.interests & (POLLIN as u32) != 0 {
                ev |= POLLIN as i16;
            }
            if r.interests & (POLLOUT as u32) != 0 {
                ev |= POLLOUT as i16;
            }
            pfds.push(WSAPOLLFD {
                fd: r.socket,
                events: ev,
                revents: 0,
            });
        }

        let timeout = timeout_ms.unwrap_or(-1);
        let nfds = pfds.len() as u32;

        // SAFETY: pfds has capacity for nfds elements, WSAPoll writes revents
        let res = unsafe { WSAPoll(pfds.as_mut_ptr(), nfds, timeout) };
        if res < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut ready_count = 0;
        let cap = events.capacity();

        for (idx, pfd) in pfds.iter().enumerate() {
            if pfd.revents != 0 && ready_count < cap {
                let reg = regs[idx];
                events.raw[ready_count] = WinEvent {
                    token: reg.token,
                    events: pfd.revents as u32,
                };
                ready_count += 1;
            }
        }

        events.len = ready_count;
        Ok(())
    }
}
