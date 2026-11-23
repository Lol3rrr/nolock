use alloc::boxed::Box;
use core::{fmt::Debug, ptr::NonNull};

mod level;
use level::Level;

mod entry;
use entry::Entry;

mod ptr;
use ptr::{CustomPtr, PtrTarget};

use crate::thread_data::{StorageBackend, cleanup::Cleanup};

/// A Lock-Free Trie that can be used as the StorageBackend for Thread-Local-Data
pub struct Trie<T> {
    // The Pointer to the first Level
    initial_ptr: NonNull<Level<T>>,
}

impl<T> Debug for Trie<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Safety:
        // This is save to do because we create the Pointer when creating the
        // Trie meaning it is always going to be a valid pointer to a Level.
        // The Memory being pointed to is also still valid because we only
        // deallocate it once the Trie is dropped.
        let initial_level = unsafe { self.initial_ptr.as_ref() };
        write!(f, "Trie ({:?})", initial_level)
    }
}

impl<T> Trie<T> {
    /// Creates a new Trie instance
    pub fn new() -> Self {
        let initial_level = Level::new(0, 3, core::ptr::null());

        Self {
            initial_ptr: Box::into_non_null(initial_level),
        }
    }
}

impl<T> StorageBackend<T> for Trie<T> where T: 'static {
    fn get(&self, id: u64) -> Option<&T> {
        // This simply "forwards" the get to the first initial Level of the
        // Trie

        // Safety:
        // This is save to do because we create the Pointer when creating the
        // Trie meaning it is always going to be a valid pointer to a Level.
        // The Memory being pointed to is also still valid because we only
        // deallocate it once the Trie is dropped.
        let level = unsafe { self.initial_ptr.as_ref() };
        level.get(id)
    }

    fn insert(&self, id: u64, data: T, cleanup: &mut Vec<Cleanup>) -> &T {
        // This simply "forwards" the insert to the first initial Level of the
        // Trie
        
        let mut data_ptr = Box::into_raw(Box::new(data));

        cleanup.push(Cleanup::new(data_ptr, |ptr| {
            drop(unsafe { Box::from_raw(ptr) });
        }));

        // Safety:
        // This is save to do because we create the Pointer when creating the
        // Trie meaning it is always going to be a valid pointer to a Level.
        // The Memory being pointed to is also still valid because we only
        // deallocate it once the Trie is dropped.
        let level = unsafe { self.initial_ptr.as_ref() };
        level.insert(id, data_ptr)
    }
}

impl<T> Default for Trie<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for Trie<T> {
    fn drop(&mut self) {
        unsafe { drop(Box::from_non_null(self.initial_ptr)) };
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

        let mut cleanup = Vec::new();
        let value = trie.insert(123, 13, &mut cleanup);
        assert_eq!(13, *value);
    }

    #[test]
    fn insert_get() {
        let trie = Trie::<usize>::new();

        let mut cleanup = Vec::new();
        let value = trie.insert(123, 13, &mut cleanup);
        assert_eq!(13, *value);

        let result = trie.get(123);
        assert_eq!(Some(&13), result);
    }

    #[test]
    fn insert_get_colliding() {
        let trie = Trie::<usize>::new();

        let mut cleanup = Vec::new();

        assert_eq!(13, *trie.insert(0x1234, 13, &mut cleanup));
        assert_eq!(14, *trie.insert(0x1334, 14, &mut cleanup));
        assert_eq!(15, *trie.insert(0x1434, 15, &mut cleanup));

        assert_eq!(Some(&13), trie.get(0x1234));
        assert_eq!(Some(&14), trie.get(0x1334));
        assert_eq!(Some(&15), trie.get(0x1434));
    }
}
