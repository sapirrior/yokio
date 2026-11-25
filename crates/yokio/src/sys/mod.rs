//! Platform-specific OS readiness primitives and event loop mechanisms.

pub(crate) mod doorbell;
pub(crate) mod events;
pub(crate) mod flags;
pub(crate) mod types;

pub(crate) use flags::os_flags;
#[allow(unused_imports)]
pub(crate) use types::{AsRawFd, RawFd};

#[cfg(target_os = "linux")]
pub(crate) mod epoll;
#[cfg(target_os = "linux")]
pub(crate) use epoll::Selector;

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
pub(crate) mod kqueue;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
pub(crate) use kqueue::Selector;

#[cfg(target_os = "windows")]
pub(crate) mod windows;
#[cfg(target_os = "windows")]
pub(crate) use windows::Selector;

#[allow(unused_imports)]
pub(crate) use doorbell::Doorbell;
#[allow(unused_imports)]
pub(crate) use events::Events;

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::key::Key;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    #[allow(clippy::cast_sign_loss)]
    fn epoll_doorbell_roundtrip() {
        let selector = Selector::new().expect("failed to create selector");
        let doorbell = Arc::new(Doorbell::new().expect("failed to create doorbell"));

        selector
            .register(doorbell.raw_fd(), Key::DOORBELL_TOKEN, os_flags::DOORBELL)
            .expect("failed to register doorbell");

        let mut events = Events::with_capacity(16);

        // Initially no events
        selector.poll(&mut events, Some(0)).expect("poll failed");
        assert_eq!(events.len(), 0);

        // Ring from a background thread using the safe Doorbell::ring() API
        let handle = std::thread::spawn({
            let doorbell = Arc::clone(&doorbell);
            move || {
                std::thread::sleep(Duration::from_millis(10));
                doorbell.ring();
            }
        });

        // Poll with a 500ms timeout
        selector.poll(&mut events, Some(500)).expect("poll failed");
        assert_eq!(events.len(), 1);

        let (token, ready) = events.iter().next().unwrap();
        assert_eq!(token, Key::DOORBELL_TOKEN);
        assert_ne!(ready & os_flags::READABLE, 0);

        doorbell.drain();
        handle.join().unwrap();
    }

    #[test]
    fn epoll_zero_timeout_returns_immediately() {
        let selector = Selector::new().expect("failed to create selector");
        let mut events = Events::with_capacity(16);

        let t0 = Instant::now();
        selector.poll(&mut events, Some(0)).expect("poll failed");
        let elapsed = t0.elapsed();

        assert_eq!(events.len(), 0);
        assert!(elapsed < Duration::from_millis(50));
    }

    #[test]
    fn epoll_timeout_honored() {
        let selector = Selector::new().expect("failed to create selector");
        let mut events = Events::with_capacity(16);

        let t0 = Instant::now();
        selector.poll(&mut events, Some(30)).expect("poll failed");
        let elapsed = t0.elapsed();

        assert_eq!(events.len(), 0);
        assert!(elapsed >= Duration::from_millis(25));
    }

    #[test]
    fn doorbell_coalescing() {
        let doorbell = Doorbell::new().expect("failed to create doorbell");
        for _ in 0..10 {
            doorbell.ring();
        }
        doorbell.drain();
        // Subsequent drain is non-blocking and safe
        doorbell.drain();
    }
}
