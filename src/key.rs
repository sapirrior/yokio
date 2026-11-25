//! Generational key identifiers for stored tasks, timers, and watchers.

#[allow(dead_code)]
/// A packed generational key consisting of a 32-bit slot index and a 32-bit generation.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub(crate) struct Key {
    /// Slot index within the slab.
    pub(crate) index: u32,
    /// Generation counter to detect stale handles.
    pub(crate) gen: u32,
}

#[allow(dead_code)]
impl Key {
    /// Sentinel index indicating absence or end of a linked list.
    pub(crate) const NIL: u32 = u32::MAX;

    /// Special token reserved for cross-thread doorbell wakeups.
    pub(crate) const DOORBELL_TOKEN: u64 = u64::MAX;

    /// Packs the generation and slot index into a 64-bit integer.
    #[inline]
    pub(crate) fn to_u64(self) -> u64 {
        (u64::from(self.gen) << 32) | u64::from(self.index)
    }

    /// Unpacks a 64-bit integer into a `Key`.
    #[inline]
    pub(crate) fn from_u64(v: u64) -> Self {
        Self {
            // Explicit truncate cast: index is stored in the lower 32 bits.
            index: v as u32,
            // Explicit truncate cast: generation is stored in the upper 32 bits.
            gen: (v >> 32) as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_u64_roundtrip() {
        let key = Key {
            index: 42,
            gen: 100,
        };
        let packed = key.to_u64();
        assert_eq!(Key::from_u64(packed), key);
    }

    #[test]
    fn doorbell_token_is_distinct_from_valid_keys() {
        // Since slabs enforce index < u32::MAX - 1, a valid key never equals DOORBELL_TOKEN.
        let max_valid_key = Key {
            index: u32::MAX - 2,
            gen: u32::MAX,
        };
        assert_ne!(max_valid_key.to_u64(), Key::DOORBELL_TOKEN);
    }
}
