//! Monotonic clock and tick conversion utilities.

use std::time::{Duration, Instant};

#[cfg(feature = "test-util")]
use std::cell::Cell;

/// Nanoseconds per timer tick (1 millisecond).
pub(crate) const TICK_NS: u64 = 1_000_000;

/// Monotonic clock used to drive timers and loop iterations.
#[derive(Debug)]
pub(crate) enum Clock {
    /// Wall-clock system monotonic time.
    System { start: Instant },
    /// Deterministic manual clock for testing and simulation.
    #[cfg(feature = "test-util")]
    Manual { start: Instant, ticks: Cell<u64> },
}

#[allow(dead_code)]
impl Clock {
    /// Creates a new system clock anchored at the current instant.
    pub(crate) fn system() -> Self {
        Self::System {
            start: Instant::now(),
        }
    }

    /// Creates a manual clock starting at tick 0.
    #[cfg(feature = "test-util")]
    pub(crate) fn manual() -> Self {
        Self::Manual {
            start: Instant::now(),
            ticks: Cell::new(0),
        }
    }

    /// Current tick count (floor of elapsed time divided by `TICK_NS`).
    pub(crate) fn now_tick(&self) -> u64 {
        match self {
            Self::System { start } => {
                let elapsed = Instant::now().saturating_duration_since(*start);
                // Cast safety: nanoseconds / 1_000_000 fits in u64 for > 500 years.
                (elapsed.as_nanos() / u128::from(TICK_NS)) as u64
            }
            #[cfg(feature = "test-util")]
            Self::Manual { ticks, .. } => ticks.get(),
        }
    }

    /// Current instant corresponding to the clock's state.
    pub(crate) fn instant(&self) -> Instant {
        match self {
            Self::System { .. } => Instant::now(),
            #[cfg(feature = "test-util")]
            Self::Manual { start, ticks } => *start + Duration::from_millis(ticks.get()),
        }
    }

    /// Converts a tick count to its corresponding `Instant`.
    pub(crate) fn tick_to_instant(&self, t: u64) -> Instant {
        let start = match self {
            Self::System { start } => *start,
            #[cfg(feature = "test-util")]
            Self::Manual { start, .. } => *start,
        };
        start + Duration::from_nanos(t.saturating_mul(TICK_NS))
    }

    /// Converts an absolute `Instant` into a tick count, rounding UP to never fire early.
    pub(crate) fn instant_to_tick_ceil(&self, i: Instant) -> u64 {
        let start = match self {
            Self::System { start } => *start,
            #[cfg(feature = "test-util")]
            Self::Manual { start, .. } => *start,
        };
        if i <= start {
            return 0;
        }
        let nanos = i.duration_since(start).as_nanos();
        let ticks = (nanos + u128::from(TICK_NS - 1)) / u128::from(TICK_NS);
        u64::try_from(ticks).unwrap_or(u64::MAX / 2)
    }

    /// Converts a `Duration` into a tick count, rounding UP (minimum 1 ms if duration > 0).
    pub(crate) fn duration_to_ticks_ceil(d: Duration) -> u64 {
        let nanos = d.as_nanos();
        if nanos == 0 {
            return 0;
        }
        let ticks = (nanos + u128::from(TICK_NS - 1)) / u128::from(TICK_NS);
        u64::try_from(ticks).unwrap_or(u64::MAX / 2)
    }

    /// Whether this clock is manually advanced.
    pub(crate) fn is_manual(&self) -> bool {
        match self {
            Self::System { .. } => false,
            #[cfg(feature = "test-util")]
            Self::Manual { .. } => true,
        }
    }

    /// Advances the manual clock by the given duration.
    #[cfg(feature = "test-util")]
    pub(crate) fn advance(&self, d: Duration) {
        if let Self::Manual { ticks, .. } = self {
            let add_ticks = Self::duration_to_ticks_ceil(d);
            ticks.set(ticks.get().saturating_add(add_ticks));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_to_ticks_ceil_rounding() {
        assert_eq!(Clock::duration_to_ticks_ceil(Duration::ZERO), 0);
        assert_eq!(Clock::duration_to_ticks_ceil(Duration::from_nanos(1)), 1);
        assert_eq!(
            Clock::duration_to_ticks_ceil(Duration::from_nanos(999_999)),
            1
        );
        assert_eq!(Clock::duration_to_ticks_ceil(Duration::from_millis(1)), 1);
        assert_eq!(
            Clock::duration_to_ticks_ceil(Duration::from_nanos(1_000_001)),
            2
        );
        assert_eq!(
            Clock::duration_to_ticks_ceil(Duration::from_millis(250)),
            250
        );
    }

    #[test]
    fn system_clock_advances() {
        let clock = Clock::system();
        let t0 = clock.now_tick();
        std::thread::sleep(Duration::from_millis(5));
        let t1 = clock.now_tick();
        assert!(t1 >= t0);
    }
}
