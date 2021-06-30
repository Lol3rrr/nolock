//! A lock-free concurrent HashTrieMap
//!
//! # Reference:
//! * [A Lock-Free Hash Trie Design for Concurrent Tabled Logic Programs](https://link.springer.com/content/pdf/10.1007/s10766-014-0346-1.pdf)

use std::{
    collections::hash_map::RandomState,
    fmt::Debug,
    hash::{BuildHasher, Hash, Hasher},
    marker::PhantomData,
    ops::Deref,
};

mod hashlevel;
mod mptr;
use hashlevel::{Entry, HashLevel};

use crate::hazard_ptr;

/// TODO
#[derive(Debug)]
pub struct RefValue<K, V> {
    guard: hazard_ptr::Guard<Entry<K, V>>,
}

impl<K, V> RefValue<K, V> {
    /// TODO
    pub fn value(&self) -> &V {
        &self.guard.value
    }
}

impl<K, V> AsRef<V> for RefValue<K, V> {
    fn as_ref(&self) -> &V {
        self.value()
    }
}

impl<K, V> PartialEq for RefValue<K, V>
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.value().eq(other.value())
    }
}

impl<K, V> PartialEq<V> for RefValue<K, V>
where
    V: PartialEq,
{
    fn eq(&self, other: &V) -> bool {
        self.value().eq(other)
    }
}

/// A Concurrent and Lock-Free HashTrieMap
pub struct HashTrieMap<K, V, H = RandomState> {
    initial_level: HashLevel<K, V, 4>,
    build_hasher: H,
    _marker: PhantomData<H>,
}

impl<K, V> HashTrieMap<K, V, RandomState> {
    /// Creates a new HashTrieMap
    pub fn new() -> Self {
        Self::with_build_hasher(std::collections::hash_map::RandomState::new())
    }
}

impl<K, V> Default for HashTrieMap<K, V, RandomState> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, H> Debug for HashTrieMap<K, V, H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HashTrieMap ()")
    }
}

impl<K, V, H> HashTrieMap<K, V, H>
where
    H: BuildHasher,
{
    /// TODO
    pub fn with_build_hasher(build_hasher: H) -> Self {
        let start_level = HashLevel::new(std::ptr::null(), 0);

        Self {
            initial_level: *start_level,
            build_hasher,
            _marker: PhantomData,
        }
    }
}

impl<K, V, H> HashTrieMap<K, V, H>
where
    K: Hash + Eq + Debug,
    H: BuildHasher,
    V: Clone + Debug,
{
    /// Inserts the given Key and Value into the Map
    pub fn insert(&self, key: K, value: V) {
        let mut hasher = self.build_hasher.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        self.initial_level.insert(hash, key, value);
    }

    /// Clones out a value from the Hash-Trie-Map
    pub fn get(&self, key: K) -> Option<RefValue<K, V>> {
        let mut hasher = self.build_hasher.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        self.initial_level.get(hash, &key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get() {
        let map: HashTrieMap<String, usize, RandomState> = HashTrieMap::new();

        map.insert("test".to_owned(), 123);
        let result = map.get("test".to_owned());
        assert_eq!(true, result.is_some());
        assert_eq!(result.unwrap(), 123);
    }
}
