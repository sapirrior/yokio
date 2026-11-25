//! Hierarchical hashed timing wheel.
//!
//! Implements a 6-level x 64-slot hierarchical timing wheel with 1 ms resolution,
//! providing amortized O(1) insertion, cancellation, and expiration.

#![allow(clippy::verbose_bit_mask, clippy::if_not_else)]

use crate::key::Key;
use crate::task::callback::{InlineFnMut, InlineFnOnce};
use crate::task::slab::Slab;

/// Number of levels in the hierarchical wheel.
const LEVELS: usize = 6;
/// Number of slots per wheel level (64 slots, 6 bits).
const SLOTS: usize = 64;

/// Strategy for handling missed ticks on recurring timers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Missed {
    /// Skip missed ticks and align to the next future period boundary.
    #[default]
    Skip,
    /// Fire immediately for missed ticks in a burst.
    Burst,
    /// Delay by one period from the actual fire time.
    Delay,
}

/// State of a registered timer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum TimerState {
    Armed,
    Expired,
}

/// Callback payload for a timer.
pub(crate) enum TimerCb {
    Once(Option<InlineFnOnce<()>>),
    Repeat(Option<InlineFnMut<()>>),
}

/// Internal timer entry stored within the wheel's slab.
pub(crate) struct TimerEntry {
    /// Absolute deadline in ticks when the timer should fire.
    pub(crate) when: u64,
    /// Period in ticks for recurring timers (0 = one-shot).
    pub(crate) period: u64,
    /// Missed tick behavior for recurring timers.
    pub(crate) missed: Missed,
    /// Number of remaining firings (`u32::MAX` = infinite).
    pub(crate) remaining: u32,
    /// Current timer state.
    pub(crate) state: TimerState,
    /// Whether this timer keeps the event loop alive.
    pub(crate) refd: bool,
    /// User callback closure.
    pub(crate) cb: TimerCb,
    level: u8,
    slot: u8,
    prev: u32,
    next: u32,
}

#[derive(Debug)]
struct Level {
    occupied: u64,
    heads: [u32; SLOTS],
    tails: [u32; SLOTS],
}

impl Level {
    fn new() -> Self {
        Self {
            occupied: 0,
            heads: [Key::NIL; SLOTS],
            tails: [Key::NIL; SLOTS],
        }
    }
}

/// Hierarchical hashed timing wheel managing timer scheduling and expiration.
pub(crate) struct TimerWheel {
    slab: Slab<TimerEntry>,
    elapsed: u64,
    levels: [Level; LEVELS],
}

#[allow(dead_code)]
impl TimerWheel {
    /// Creates a new timing wheel with pre-allocated slab capacity.
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            slab: Slab::with_capacity(capacity),
            elapsed: 0,
            levels: [
                Level::new(),
                Level::new(),
                Level::new(),
                Level::new(),
                Level::new(),
                Level::new(),
            ],
        }
    }

    /// Current tick count of the timing wheel.
    #[inline]
    pub(crate) fn elapsed(&self) -> u64 {
        self.elapsed
    }

    /// Number of active and expired timers in the slab.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.slab.len()
    }

    /// Whether there are no timers in the slab.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.slab.is_empty()
    }

    /// Inserts a new timer into the wheel, returning its generational key.
    pub(crate) fn insert(&mut self, mut entry: TimerEntry) -> Key {
        // Enforce: timer must be in the future (at least elapsed + 1)
        if entry.when <= self.elapsed {
            entry.when = self.elapsed.saturating_add(1);
        }
        entry.state = TimerState::Armed;
        let (lvl, slot) = Self::slot_for(entry.when, self.elapsed);
        entry.level = lvl;
        entry.slot = slot;
        entry.prev = Key::NIL;
        entry.next = Key::NIL;

        let key = self.slab.insert(entry);
        self.link_entry(key.index, lvl, slot);
        key
    }

    /// Cancels a timer, unlinking it and removing it from the slab.
    pub(crate) fn cancel(&mut self, k: Key) -> Option<TimerEntry> {
        let entry = self.slab.get(k)?;
        if entry.state == TimerState::Armed {
            let lvl = entry.level;
            let slot = entry.slot;
            self.unlink_entry(k.index, lvl, slot);
        }
        self.slab.remove(k)
    }

    /// Rearms an existing timer with a new deadline.
    pub(crate) fn rearm(&mut self, k: Key, mut when: u64) -> bool {
        if when <= self.elapsed {
            when = self.elapsed.saturating_add(1);
        }
        let Some(entry) = self.slab.get(k) else {
            return false;
        };
        let old_lvl = entry.level;
        let old_slot = entry.slot;
        let was_armed = entry.state == TimerState::Armed;

        if was_armed {
            self.unlink_entry(k.index, old_lvl, old_slot);
        }

        let (new_lvl, new_slot) = Self::slot_for(when, self.elapsed);
        if let Some(entry) = self.slab.get_mut(k) {
            entry.when = when;
            entry.level = new_lvl;
            entry.slot = new_slot;
            entry.state = TimerState::Armed;
            entry.prev = Key::NIL;
            entry.next = Key::NIL;
        }

        self.link_entry(k.index, new_lvl, new_slot);
        true
    }

    /// Borrows a timer entry by key.
    pub(crate) fn get(&self, k: Key) -> Option<&TimerEntry> {
        self.slab.get(k)
    }

    /// Mutably borrows a timer entry by key.
    pub(crate) fn get_mut(&mut self, k: Key) -> Option<&mut TimerEntry> {
        self.slab.get_mut(k)
    }

    /// Calculates the earliest deadline among all armed timers, if any.
    pub(crate) fn next_deadline(&self) -> Option<u64> {
        if self.slab.is_empty() {
            return None;
        }
        let mut earliest = None;
        for lvl in 0..LEVELS {
            let occ = self.levels[lvl].occupied;
            if occ != 0 {
                for s in 0..SLOTS {
                    if (occ & (1 << s)) != 0 {
                        let mut curr = self.levels[lvl].heads[s];
                        while curr != Key::NIL {
                            if let Some(EntryRef(when)) = self.entry_when(curr) {
                                earliest = match earliest {
                                    Some(min) if min < when => Some(min),
                                    _ => Some(when),
                                };
                            }
                            curr = self.entry_next(curr);
                        }
                    }
                }
            }
        }
        earliest
    }

    /// Advances the wheel to `now`, cascading upper levels and collecting expired keys.
    pub(crate) fn poll_expired(&mut self, now: u64, expired: &mut Vec<Key>) {
        if self.slab.is_empty() {
            self.elapsed = now;
            return;
        }

        while self.elapsed < now {
            self.elapsed += 1;
            let current = self.elapsed;

            // Cascade higher levels on radix wraparounds
            if (current & 63) == 0 {
                self.cascade_level(1, ((current >> 6) & 63) as u8);
            }
            if (current & 4095) == 0 {
                self.cascade_level(2, ((current >> 12) & 63) as u8);
            }
            if (current & 262_143) == 0 {
                self.cascade_level(3, ((current >> 18) & 63) as u8);
            }
            if (current & 16_777_215) == 0 {
                self.cascade_level(4, ((current >> 24) & 63) as u8);
            }
            if (current & 1_073_741_823) == 0 {
                self.cascade_level(5, ((current >> 30) & 63) as u8);
            }

            // Drain expired timers from Level 0
            let slot0 = (current & 63) as usize;
            let mut curr = self.levels[0].heads[slot0];
            self.levels[0].heads[slot0] = Key::NIL;
            self.levels[0].tails[slot0] = Key::NIL;
            self.levels[0].occupied &= !(1 << slot0);

            while curr != Key::NIL {
                let next_idx = self.entry_next(curr);
                let Some((gen, _when)) = self.entry_gen_when(curr) else {
                    curr = next_idx;
                    continue;
                };

                let key = Key { index: curr, gen };
                if let Some(entry) = self.slab.get_mut(key) {
                    entry.state = TimerState::Expired;
                    entry.prev = Key::NIL;
                    entry.next = Key::NIL;
                }
                expired.push(key);
                curr = next_idx;
            }
        }
    }

    fn cascade_level(&mut self, lvl: usize, slot: u8) {
        let s = slot as usize;
        let mut curr = self.levels[lvl].heads[s];
        self.levels[lvl].heads[s] = Key::NIL;
        self.levels[lvl].tails[s] = Key::NIL;
        self.levels[lvl].occupied &= !(1 << s);

        while curr != Key::NIL {
            let next_idx = self.entry_next(curr);
            if let Some((gen, when)) = self.entry_gen_when(curr) {
                let key = Key { index: curr, gen };
                let (new_lvl, new_slot) = Self::slot_for(when, self.elapsed);
                if let Some(entry) = self.slab.get_mut(key) {
                    entry.level = new_lvl;
                    entry.slot = new_slot;
                    entry.prev = Key::NIL;
                    entry.next = Key::NIL;
                }
                self.link_entry(curr, new_lvl, new_slot);
            }
            curr = next_idx;
        }
    }

    fn link_entry(&mut self, idx: u32, lvl: u8, slot: u8) {
        let l = lvl as usize;
        let s = slot as usize;
        let old_tail = self.levels[l].tails[s];
        if old_tail == Key::NIL {
            self.levels[l].heads[s] = idx;
            self.levels[l].tails[s] = idx;
            self.levels[l].occupied |= 1 << s;
            if let Some(entry) = self.entry_mut(idx) {
                entry.prev = Key::NIL;
                entry.next = Key::NIL;
            }
        } else {
            if let Some(tail_entry) = self.entry_mut(old_tail) {
                tail_entry.next = idx;
            }
            if let Some(entry) = self.entry_mut(idx) {
                entry.prev = old_tail;
                entry.next = Key::NIL;
            }
            self.levels[l].tails[s] = idx;
        }
    }

    fn unlink_entry(&mut self, idx: u32, lvl: u8, slot: u8) {
        let l = lvl as usize;
        let s = slot as usize;
        let (prev, next) = match self.entry_ref(idx) {
            Some(e) => (e.prev, e.next),
            None => return,
        };

        if prev != Key::NIL {
            if let Some(p) = self.entry_mut(prev) {
                p.next = next;
            }
        } else {
            self.levels[l].heads[s] = next;
        }

        if next != Key::NIL {
            if let Some(n) = self.entry_mut(next) {
                n.prev = prev;
            }
        } else {
            self.levels[l].tails[s] = prev;
        }

        if self.levels[l].heads[s] == Key::NIL {
            self.levels[l].occupied &= !(1 << s);
        }

        if let Some(entry) = self.entry_mut(idx) {
            entry.prev = Key::NIL;
            entry.next = Key::NIL;
        }
    }

    fn slot_for(when: u64, elapsed: u64) -> (u8, u8) {
        let diff = when.saturating_sub(elapsed);
        if diff < 64 {
            let slot = (when & 63) as u8;
            (0, slot)
        } else if diff < 4096 {
            let slot = ((when >> 6) & 63) as u8;
            (1, slot)
        } else if diff < 262_144 {
            let slot = ((when >> 12) & 63) as u8;
            (2, slot)
        } else if diff < 16_777_216 {
            let slot = ((when >> 18) & 63) as u8;
            (3, slot)
        } else if diff < 1_073_741_824 {
            let slot = ((when >> 24) & 63) as u8;
            (4, slot)
        } else {
            let slot = ((when >> 30) & 63) as u8;
            (5, slot)
        }
    }

    fn entry_next(&self, idx: u32) -> u32 {
        self.entry_ref(idx).map_or(Key::NIL, |e| e.next)
    }

    fn entry_when(&self, idx: u32) -> Option<EntryRef> {
        self.entry_ref(idx).map(|e| EntryRef(e.when))
    }

    fn entry_gen_when(&self, idx: u32) -> Option<(u32, u64)> {
        self.slab_entry_gen_when(idx)
    }

    fn entry_ref(&self, idx: u32) -> Option<&TimerEntry> {
        self.slab_entry_ref(idx)
    }

    fn entry_mut(&mut self, idx: u32) -> Option<&mut TimerEntry> {
        self.slab_entry_mut(idx)
    }

    fn slab_entry_ref(&self, idx: u32) -> Option<&TimerEntry> {
        self.slab.get_by_index(idx)
    }

    fn slab_entry_mut(&mut self, idx: u32) -> Option<&mut TimerEntry> {
        self.slab.get_mut_by_index(idx)
    }

    fn slab_entry_gen_when(&self, idx: u32) -> Option<(u32, u64)> {
        self.slab.get_gen_and_with(idx, |e| e.when)
    }
}

struct EntryRef(u64);

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_entry(when: u64) -> TimerEntry {
        TimerEntry {
            when,
            period: 0,
            missed: Missed::Skip,
            remaining: 1,
            state: TimerState::Armed,
            refd: true,
            cb: TimerCb::Once(None),
            level: 0,
            slot: 0,
            prev: Key::NIL,
            next: Key::NIL,
        }
    }

    #[test]
    #[allow(clippy::assert_is_empty)]
    fn single_timer_fires_at_deadline() {
        let mut wheel = TimerWheel::new(4);
        let key = wheel.insert(dummy_entry(10));
        assert_eq!(wheel.len(), 1);

        let mut expired = Vec::new();
        wheel.poll_expired(9, &mut expired);
        assert!(expired.is_empty());
        assert_eq!(wheel.elapsed(), 9);

        wheel.poll_expired(10, &mut expired);
        assert_eq!(expired, vec![key]);
        assert_eq!(wheel.get(key).unwrap().state, TimerState::Expired);
    }

    #[test]
    fn multiple_timers_cascade_and_fire_in_order() {
        let mut wheel = TimerWheel::new(8);
        // Level 0 timer
        let k1 = wheel.insert(dummy_entry(20));
        // Level 1 timer (diff = 100 >= 64)
        let k2 = wheel.insert(dummy_entry(100));
        // Level 2 timer (diff = 5000 >= 4096)
        let k3 = wheel.insert(dummy_entry(5000));

        let mut expired = Vec::new();
        wheel.poll_expired(20, &mut expired);
        assert_eq!(expired, vec![k1]);

        wheel.poll_expired(99, &mut expired);
        assert_eq!(expired, vec![k1]);

        wheel.poll_expired(100, &mut expired);
        assert_eq!(expired, vec![k1, k2]);

        wheel.poll_expired(5000, &mut expired);
        assert_eq!(expired, vec![k1, k2, k3]);
    }

    #[test]
    fn cancel_prevents_firing() {
        let mut wheel = TimerWheel::new(4);
        let k1 = wheel.insert(dummy_entry(5));
        let k2 = wheel.insert(dummy_entry(10));

        assert!(wheel.cancel(k1).is_some());
        assert_eq!(wheel.len(), 1);

        let mut expired = Vec::new();
        wheel.poll_expired(15, &mut expired);
        assert_eq!(expired, vec![k2]);
    }

    #[test]
    #[allow(clippy::assert_is_empty)]
    fn rearm_updates_deadline() {
        let mut wheel = TimerWheel::new(4);
        let k = wheel.insert(dummy_entry(5));

        assert!(wheel.rearm(k, 15));
        let mut expired = Vec::new();
        wheel.poll_expired(10, &mut expired);
        assert!(expired.is_empty());

        wheel.poll_expired(15, &mut expired);
        assert_eq!(expired, vec![k]);
    }
}
