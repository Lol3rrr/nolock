//! A lock-free concurrent HashTrieMap
//!
//! # Reference:
//! * [A Lock-Free Hash Trie Design for Concurrent Tabled Logic Programs](https://link.springer.com/content/pdf/10.1007/s10766-014-0346-1.pdf)

use std::{
    collections::hash_map::RandomState,
    fmt::Debug,
    hash::{Hash, Hasher},
    marker::PhantomData,
};

mod hashlevel;
mod mptr;
use hashlevel::{Entry, HashLevel};

/// A Concurrent and Lock-Free HashTrieMap
pub struct HashTrieMap<K, V, H = RandomState> {
    initial_level: HashLevel<K, V>,
    _marker: PhantomData<H>,
}

impl<K, V, RandomState> HashTrieMap<K, V, RandomState>
where
    K: Eq + Debug,
    V: Clone + Debug,
{
    /// Creates a new HashTrieMap
    pub fn new() -> Self {
        let start_level = HashLevel::new(std::ptr::null(), 4, 0);
        Self {
            initial_level: *start_level,
            _marker: PhantomData,
        }
    }
}

impl<K, V, RandomState> Default for HashTrieMap<K, V, RandomState>
where
    K: Eq + Debug,
    V: Clone + Debug,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V, H> HashTrieMap<K, V, H>
where
    K: Hash + Eq + Debug,
    H: Hasher + Default,
    V: Clone + Debug,
{
    /// Inserts the given Key and Value into the Map
    pub fn insert(&self, key: K, value: V) {
        let mut hasher = H::default();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        self.initial_level.insert(hash, key, value);
    }

    /// Clones out a value from the Hash-Trie-Map
    pub fn get_cloned(&self, key: K) -> Option<V> {
        let mut hasher = H::default();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        self.initial_level.get_clone(hash, &key)
    }
}
