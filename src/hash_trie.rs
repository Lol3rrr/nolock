//! A lock-free concurrent HashTrieMap
//!
//! # Reference:
//! * [A Lock-Free Hash Trie Design for Concurrent Tabled Logic Programs](https://link.springer.com/content/pdf/10.1007/s10766-014-0346-1.pdf)
//! * [Towards a Lock-Free, Fixed Size and Persistent Hash Map Design](https://repositorio.inesctec.pt/bitstream/123456789/6155/1/P-00N-B3Y.pdf)

use std::{
    collections::hash_map::RandomState,
    fmt::Debug,
    hash::{BuildHasher, Hash, Hasher},
    marker::PhantomData,
};

mod entry;
mod hashlevel;
mod mptr;
use entry::Entry;
use hashlevel::HashLevel;

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
    pub fn get(&self, key: &K) -> Option<RefValue<K, V>> {
        let mut hasher = self.build_hasher.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        self.initial_level.get(hash, key)
    }

    /// TODO
    pub fn remove(&self, key: &K) {
        let mut hasher = self.build_hasher.build_hasher();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        self.initial_level.remove_entry(hash, key);
    }
}

unsafe impl<K, V, H> Sync for HashTrieMap<K, V, H> {}
unsafe impl<K, V, H> Send for HashTrieMap<K, V, H> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_non_existing() {
        let map: HashTrieMap<String, usize> = HashTrieMap::new();

        assert_eq!(None, map.get(&"test".to_owned()));
    }

    #[test]
    fn insert_get() {
        let map: HashTrieMap<String, usize, RandomState> = HashTrieMap::new();

        map.insert("test".to_owned(), 123);
        let result = map.get(&"test".to_owned());
        assert_eq!(true, result.is_some());
        assert_eq!(result.unwrap(), 123);
    }

    #[test]
    fn insert_overwrite() {
        let map: HashTrieMap<String, usize, RandomState> = HashTrieMap::new();

        map.insert("test".to_owned(), 123);
        let result = map.get(&"test".to_owned());
        assert_eq!(true, result.is_some());
        let first_value = result.unwrap();
        assert_eq!(first_value, 123);

        map.insert("test".to_owned(), 234);
        let result = map.get(&"test".to_owned());
        assert_eq!(true, result.is_some());
        let second_value = result.unwrap();
        assert_eq!(second_value, 234);

        // Check that the first result is still valid
        assert_eq!(first_value, 123);
    }

    #[test]
    fn insert_remove() {
        let map: HashTrieMap<String, usize> = HashTrieMap::new();

        map.insert("test".to_owned(), 123);
        let result = map.get(&"test".to_owned());
        assert_eq!(true, result.is_some());
        let first_value = result.unwrap();
        assert_eq!(first_value, 123);

        map.remove(&"test".to_owned());

        assert_eq!(None, map.get(&"test".to_owned()));

        assert_eq!(first_value, 123);
    }
}
