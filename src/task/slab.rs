//! Generational slab storage for handles.
//!
//! Provides O(1) insertion, lookup, and removal while tracking generational
//! counters to detect and prevent access through stale handles.

use crate::key::Key;

#[derive(Debug)]
enum Entry<T> {
    Vacant { next_free: u32, gen: u32 },
    Occupied { gen: u32, value: T },
}

/// A generational arena with generational key validation and free-list reuse.
#[derive(Debug)]
pub(crate) struct Slab<T> {
    entries: Vec<Entry<T>>,
    free_head: u32,
    len: u32,
}

impl<T> Slab<T> {
    /// Creates a new slab with pre-allocated capacity.
    pub(crate) fn with_capacity(n: usize) -> Self {
        Self {
            entries: Vec::with_capacity(n),
            free_head: Key::NIL,
            len: 0,
        }
    }

    /// Number of occupied slots in the slab.
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether there are no occupied entries.
    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Inserts a value into the slab, returning its generational key.
    pub(crate) fn insert(&mut self, value: T) -> Key {
        self.insert_with(|_| value)
    }

    /// Inserts a value constructed by a closure that takes the newly allocated `Key`.
    pub(crate) fn insert_with(&mut self, f: impl FnOnce(Key) -> T) -> Key {
        if self.free_head == Key::NIL {
            let index = self.entries.len();
            assert!(
                index < (u32::MAX - 1) as usize,
                "yokio: slab capacity exhausted"
            );
            // Explicit truncate cast: index < u32::MAX - 1 verified above.
            let key = Key {
                index: index as u32,
                gen: 0,
            };
            let val = f(key);
            self.entries.push(Entry::Occupied { gen: 0, value: val });
            self.len += 1;
            key
        } else {
            let index = self.free_head;
            match &mut self.entries[index as usize] {
                Entry::Vacant { next_free, gen } => {
                    let key = Key { index, gen: *gen };
                    self.free_head = *next_free;
                    let val = f(key);
                    self.entries[index as usize] = Entry::Occupied {
                        gen: *gen,
                        value: val,
                    };
                    self.len += 1;
                    key
                }
                Entry::Occupied { .. } => unreachable!("corrupt slab free list"),
            }
        }
    }

    /// Borrows the value associated with the key if the key is valid and current.
    pub(crate) fn get(&self, k: Key) -> Option<&T> {
        match self.entries.get(k.index as usize)? {
            Entry::Occupied { gen, value } if *gen == k.gen => Some(value),
            _ => None,
        }
    }

    /// Borrows an occupied value directly by slot index.
    pub(crate) fn get_by_index(&self, index: u32) -> Option<&T> {
        match self.entries.get(index as usize)? {
            Entry::Occupied { value, .. } => Some(value),
            Entry::Vacant { .. } => None,
        }
    }

    /// Mutably borrows an occupied value directly by slot index.
    pub(crate) fn get_mut_by_index(&mut self, index: u32) -> Option<&mut T> {
        match self.entries.get_mut(index as usize)? {
            Entry::Occupied { value, .. } => Some(value),
            Entry::Vacant { .. } => None,
        }
    }

    /// Retrieves generation and a projected value directly by slot index.
    pub(crate) fn get_gen_and_with<R>(
        &self,
        index: u32,
        f: impl FnOnce(&T) -> R,
    ) -> Option<(u32, R)> {
        match self.entries.get(index as usize)? {
            Entry::Occupied { gen, value } => Some((*gen, f(value))),
            Entry::Vacant { .. } => None,
        }
    }

    /// Mutably borrows the value associated with the key if valid and current.
    pub(crate) fn get_mut(&mut self, k: Key) -> Option<&mut T> {
        match self.entries.get_mut(k.index as usize)? {
            Entry::Occupied { gen, value } if *gen == k.gen => Some(value),
            _ => None,
        }
    }

    /// Checks if a key points to an occupied entry with matching generation.
    pub(crate) fn contains(&self, k: Key) -> bool {
        self.get(k).is_some()
    }

    /// Removes an entry by key, bumping its generation and returning the value.
    pub(crate) fn remove(&mut self, k: Key) -> Option<T> {
        let entry = self.entries.get_mut(k.index as usize)?;
        match entry {
            Entry::Occupied { gen, .. } if *gen == k.gen => {
                let next_gen = gen.wrapping_add(1);
                let old_entry = std::mem::replace(
                    entry,
                    Entry::Vacant {
                        next_free: self.free_head,
                        gen: next_gen,
                    },
                );
                self.free_head = k.index;
                self.len -= 1;
                match old_entry {
                    Entry::Occupied { value, .. } => Some(value),
                    Entry::Vacant { .. } => unreachable!(),
                }
            }
            _ => None,
        }
    }

    /// Drains all occupied values for teardown, bumping generations and resetting the slab.
    pub(crate) fn drain_values(&mut self) -> Vec<T> {
        let mut values = Vec::with_capacity(self.len as usize);
        for entry in &mut self.entries {
            if let Entry::Occupied { gen, .. } = entry {
                let next_gen = gen.wrapping_add(1);
                let old_entry = std::mem::replace(
                    entry,
                    Entry::Vacant {
                        next_free: Key::NIL,
                        gen: next_gen,
                    },
                );
                if let Entry::Occupied { value, .. } = old_entry {
                    values.push(value);
                }
            }
        }
        self.free_head = Key::NIL;
        self.len = 0;
        values
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove() {
        let mut slab = Slab::with_capacity(4);
        let k1 = slab.insert("apple");
        let k2 = slab.insert("banana");

        assert_eq!(slab.len(), 2);
        assert_eq!(slab.get(k1), Some(&"apple"));
        assert_eq!(slab.get(k2), Some(&"banana"));

        assert_eq!(slab.remove(k1), Some("apple"));
        assert_eq!(slab.len(), 1);
        assert_eq!(slab.get(k1), None);
        assert!(!slab.contains(k1));
    }

    #[test]
    fn stale_key_after_remove_is_none() {
        let mut slab = Slab::with_capacity(2);
        let k1 = slab.insert(100);
        slab.remove(k1);

        let k2 = slab.insert(200);
        // Slot index is reused, but generation is bumped
        assert_eq!(k1.index, k2.index);
        assert_ne!(k1.gen, k2.gen);
        assert_eq!(slab.get(k1), None);
        assert_eq!(slab.get(k2), Some(&200));
    }

    #[test]
    fn free_list_lifo() {
        let mut slab = Slab::with_capacity(4);
        let k0 = slab.insert(0);
        let k1 = slab.insert(1);
        let k2 = slab.insert(2);

        slab.remove(k1);
        slab.remove(k0);

        // LIFO: removing k1 then k0 means k0 is free_head, then k1
        let k_new0 = slab.insert(10);
        let k_new1 = slab.insert(11);
        assert_eq!(k_new0.index, k0.index);
        assert_eq!(k_new1.index, k1.index);
        assert!(slab.contains(k2));
    }

    #[test]
    fn insert_with_sees_own_key() {
        let mut slab = Slab::with_capacity(2);
        let key = slab.insert_with(|k| format!("my key is {}:{}", k.index, k.gen));
        assert_eq!(
            slab.get(key),
            Some(&format!("my key is {}:{}", key.index, key.gen))
        );
    }

    #[test]
    fn drain_values_invalidates_keys() {
        let mut slab = Slab::with_capacity(2);
        let k1 = slab.insert(1);
        let k2 = slab.insert(2);

        let drained = slab.drain_values();
        assert_eq!(drained, vec![1, 2]);
        assert_eq!(slab.len(), 0);
        assert_eq!(slab.get(k1), None);
        assert_eq!(slab.get(k2), None);
    }
}
