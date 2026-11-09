//! A hash map implemented with open addressing (linear probing).
//!
//! `alloc::collections` does not provide a hash map, so this module fills the
//! gap for `no_std` programs. It supports the operations that are commonly
//! used by applications: `insert`, `get`, `get_mut`, `remove`, `iter`, etc.

use alloc::vec::Vec;
use core::borrow::Borrow;
use core::hash::{BuildHasher, Hash, Hasher};

/// The initial number of slots in the table.
const INITIAL_CAPACITY: usize = 16;

/// The table is grown when `len * 100 / capacity` exceeds this percentage.
const LOAD_FACTOR_PERCENT: usize = 75;

/// The default hasher, a FNV-1a hash with a final avalanche step.
pub struct DefaultHasher {
    hash: u64,
}

impl Default for DefaultHasher {
    fn default() -> Self {
        Self {
            hash: 0xcbf2_9ce4_8422_2325,
        }
    }
}

impl Hasher for DefaultHasher {
    fn finish(&self) -> u64 {
        // A final mixing step (from MurmurHash3) so that the low bits, which
        // are used to index the table, are well distributed.
        let mut hash = self.hash;
        hash ^= hash >> 33;
        hash = hash.wrapping_mul(0xff51_afd7_ed55_8ccd);
        hash ^= hash >> 33;
        hash
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.hash ^= byte as u64;
            self.hash = self.hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

/// The default [`BuildHasher`] used by [`HashMap`].
#[derive(Default, Clone, Copy, Debug)]
pub struct DefaultHashBuilder;

impl BuildHasher for DefaultHashBuilder {
    type Hasher = DefaultHasher;

    fn build_hasher(&self) -> DefaultHasher {
        DefaultHasher::default()
    }
}

/// A hash map implemented with open addressing.
///
/// The keys must implement [`Hash`] and [`Eq`].
pub struct HashMap<K, V, S = DefaultHashBuilder> {
    entries: Vec<Option<(K, V)>>,
    len: usize,
    hash_builder: S,
}

impl<K, V> HashMap<K, V, DefaultHashBuilder>
where
    K: Hash + Eq,
{
    /// Creates an empty `HashMap`.
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates an empty `HashMap` with at least the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_hasher_and_capacity(DefaultHashBuilder, capacity)
    }
}

impl<K, V> Default for HashMap<K, V, DefaultHashBuilder>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, S> HashMap<K, V, S> {
    /// Creates an empty `HashMap` with the given hash builder.
    pub const fn with_hasher(hash_builder: S) -> Self {
        Self {
            entries: Vec::new(),
            len: 0,
            hash_builder,
        }
    }

    /// Returns the number of elements in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the map contains no elements.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the number of elements the map can hold without reallocating.
    pub fn capacity(&self) -> usize {
        self.entries.len() * LOAD_FACTOR_PERCENT / 100
    }

    /// Removes all elements from the map.
    pub fn clear(&mut self) {
        for entry in self.entries.iter_mut() {
            *entry = None;
        }
        self.len = 0;
    }

    /// Returns an iterator over the key-value pairs of the map.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.entries.iter(),
        }
    }

    /// Returns an iterator over the keys of the map.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(k, _)| k)
    }

    /// Returns an iterator over the values of the map.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}

impl<K, V, S> HashMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Creates an empty `HashMap` with the given hash builder and capacity.
    pub fn with_hasher_and_capacity(hash_builder: S, capacity: usize) -> Self {
        let mut map = Self::with_hasher(hash_builder);
        if capacity > 0 {
            let capacity = capacity * 100 / LOAD_FACTOR_PERCENT + 1;
            map.rehash(capacity);
        }
        map
    }
}

impl<K, V, S> HashMap<K, V, S>
where
    K: Hash + Eq,
    S: BuildHasher,
{
    /// Inserts a key-value pair into the map.
    ///
    /// If the map did not have this key present, `None` is returned. Otherwise
    /// the value is updated and the old value is returned.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if self.entries.is_empty() {
            self.rehash(INITIAL_CAPACITY);
        } else if (self.len + 1) * 100 > self.entries.len() * LOAD_FACTOR_PERCENT {
            self.rehash(self.entries.len() * 2);
        }

        let mask = self.entries.len() - 1;
        let mut idx = self.hash_of(&key) as usize & mask;
        loop {
            match &mut self.entries[idx] {
                None => {
                    self.entries[idx] = Some((key, value));
                    self.len += 1;
                    return None;
                }
                Some((k, v)) => {
                    if *k == key {
                        return Some(core::mem::replace(v, value));
                    }
                    idx = (idx + 1) & mask;
                }
            }
        }
    }

    /// Returns a reference to the value corresponding to the key.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index(key).map(|idx| {
            let (_, v) = self.entries[idx].as_ref().unwrap();
            v
        })
    }

    /// Returns a mutable reference to the value corresponding to the key.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        match self.find_index(key) {
            Some(idx) => {
                let (_, v) = self.entries[idx].as_mut().unwrap();
                Some(v)
            }
            None => None,
        }
    }

    /// Returns `true` if the map contains the given key.
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_index(key).is_some()
    }

    /// Removes a key from the map, returning the value at the key if it was
    /// present.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let idx = self.find_index(key)?;
        let (_, value) = self.entries[idx].take().unwrap();
        self.len -= 1;
        // Rebuild the table to keep the probe chains valid.
        self.rehash(self.entries.len());
        Some(value)
    }

    fn find_index<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        if self.entries.is_empty() {
            return None;
        }
        let mask = self.entries.len() - 1;
        let mut idx = self.hash_of(key) as usize & mask;
        loop {
            match &self.entries[idx] {
                None => return None,
                Some((k, _)) => {
                    if k.borrow() == key {
                        return Some(idx);
                    }
                }
            }
            idx = (idx + 1) & mask;
        }
    }

    fn hash_of<Q>(&self, key: &Q) -> u64
    where
        Q: Hash + ?Sized,
    {
        let mut hasher = self.hash_builder.build_hasher();
        key.hash(&mut hasher);
        hasher.finish()
    }

    /// Rebuilds the table with a new capacity.
    fn rehash(&mut self, new_capacity: usize) {
        let new_capacity = new_capacity
            .max(INITIAL_CAPACITY)
            .next_power_of_two();

        let old_entries = core::mem::take(&mut self.entries);
        let mut entries = Vec::with_capacity(new_capacity);
        entries.resize_with(new_capacity, || None);

        let mask = new_capacity - 1;
        for entry in old_entries {
            if let Some((key, value)) = entry {
                let mut idx = self.hash_of(&key) as usize & mask;
                while entries[idx].is_some() {
                    idx = (idx + 1) & mask;
                }
                entries[idx] = Some((key, value));
            }
        }
        self.entries = entries;
    }
}

/// An iterator over the entries of a [`HashMap`].
pub struct Iter<'a, K, V> {
    inner: core::slice::Iter<'a, Option<(K, V)>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next()? {
                Some((k, v)) => return Some((k, v)),
                None => continue,
            }
        }
    }
}

impl<'a, K, V> IntoIterator for &'a HashMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
