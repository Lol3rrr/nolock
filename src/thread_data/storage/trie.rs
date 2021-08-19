mod level;
use std::fmt::Debug;

use level::Level;

mod entry;
use entry::Entry;

mod ptr;
use ptr::{CustomPtr, PtrTarget};

use crate::thread_data::StorageBackend;

/// TODO
pub struct Trie<T> {
    initial_ptr: *mut Level<T>,
}

impl<T> Debug for Trie<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let initial_level = unsafe { &*self.initial_ptr };
        write!(f, "Trie ({:?})", initial_level)
    }
}

impl<T> Trie<T> {
    /// Creates a new Trie instance
    pub fn new() -> Self {
        let initial_level = Level::new(0, 3, std::ptr::null());

        Self {
            initial_ptr: Box::into_raw(initial_level),
        }
    }
}

impl<T> StorageBackend<T> for Trie<T> {
    fn get(&self, id: u64) -> Option<&T> {
        let level = unsafe { &*self.initial_ptr };
        level.get(id)
    }

    fn insert(&self, id: u64, data: T) -> &T {
        let level = unsafe { &*self.initial_ptr };
        level.insert(id, data)
    }
}

impl<T> Drop for Trie<T> {
    fn drop(&mut self) {
        unsafe { Box::from_raw(self.initial_ptr) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        Trie::<usize>::new();
    }

    #[test]
    fn get_empty() {
        let trie = Trie::<usize>::new();

        assert_eq!(None, trie.get(123));
        drop(trie);
    }

    #[test]
    fn insert() {
        let trie = Trie::<usize>::new();

        let value = trie.insert(123, 13);
        assert_eq!(13, *value);
    }

    #[test]
    fn insert_get() {
        let trie = Trie::<usize>::new();

        let value = trie.insert(123, 13);
        assert_eq!(13, *value);

        let result = trie.get(123);
        assert_eq!(Some(&13), result);
    }

    #[test]
    fn insert_get_colliding() {
        let trie = Trie::<usize>::new();

        assert_eq!(13, *trie.insert(0x1234, 13));
        assert_eq!(14, *trie.insert(0x1334, 14));
        assert_eq!(15, *trie.insert(0x1434, 15));

        assert_eq!(Some(&13), trie.get(0x1234));
        assert_eq!(Some(&14), trie.get(0x1334));
        assert_eq!(Some(&15), trie.get(0x1434));
    }
}
