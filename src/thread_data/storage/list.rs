use std::{fmt::Debug, sync::atomic};

use crate::thread_data::StorageBackend;

struct Entry<T> {
    id: u64,
    data: T,
    next: atomic::AtomicPtr<Self>,
}

impl<T> Debug for Entry<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let next_ptr = self.next.load(atomic::Ordering::Acquire);

        if next_ptr.is_null() {
            write!(f, "Entry ({:?}) -> X", self.data)
        } else {
            let next = unsafe { &*next_ptr };
            write!(f, "Entry ({:?}) -> {:?}", self.data, next)
        }
    }
}

/// A Lock-Free Linked-List
pub struct List<T> {
    entries: atomic::AtomicPtr<Entry<T>>,
}

impl<T> Debug for List<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let initial_entry_ptr = self.entries.load(atomic::Ordering::Acquire);
        if initial_entry_ptr.is_null() {
            return write!(f, "List ()");
        }

        let initial_entry = unsafe { &*initial_entry_ptr };

        write!(f, "List ({:?})", initial_entry)
    }
}

impl<T> List<T> {
    /// Creates a new empty Instance
    pub fn new() -> Self {
        Self {
            entries: atomic::AtomicPtr::new(std::ptr::null_mut()),
        }
    }
}

impl<T> StorageBackend<T> for List<T> {
    fn get(&self, id: u64) -> Option<&T> {
        let head_ptr = self.entries.load(atomic::Ordering::SeqCst);
        if head_ptr.is_null() {
            return None;
        }

        let mut current = unsafe { &*head_ptr };
        loop {
            if current.id == id {
                return Some(&current.data);
            }

            let next_ptr = current.next.load(atomic::Ordering::SeqCst);
            if next_ptr.is_null() {
                return None;
            }

            current = unsafe { &*next_ptr };
        }
    }
    fn insert(&self, id: u64, data: T) -> &T {
        let new_entry_ptr = Box::into_raw(Box::new(Entry {
            id,
            data,
            next: atomic::AtomicPtr::new(std::ptr::null_mut()),
        }));
        let new_entry = unsafe { &*new_entry_ptr };

        let mut head_ptr = self.entries.load(atomic::Ordering::SeqCst);
        if head_ptr.is_null() {
            match self.entries.compare_exchange(
                std::ptr::null_mut(),
                new_entry_ptr,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
            ) {
                Ok(_) => return &new_entry.data,
                Err(other_ptr) => {
                    head_ptr = other_ptr;
                }
            };
        }

        let mut current = unsafe { &*head_ptr };
        loop {
            let next_ptr = current.next.load(atomic::Ordering::SeqCst);

            if next_ptr.is_null() {
                match current.next.compare_exchange(
                    std::ptr::null_mut(),
                    new_entry_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => return &new_entry.data,
                    Err(ptr) => {
                        current = unsafe { &*ptr };
                        continue;
                    }
                };
            } else {
                current = unsafe { &*next_ptr };
            }
        }
    }
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for List<T> {
    fn drop(&mut self) {
        let head_ptr = self.entries.load(atomic::Ordering::SeqCst);
        if head_ptr.is_null() {
            return;
        }

        let mut current = unsafe { Box::from_raw(head_ptr) };
        loop {
            let next_ptr = current.next.load(atomic::Ordering::SeqCst);
            if next_ptr.is_null() {
                break;
            }

            current = unsafe { Box::from_raw(next_ptr) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_storage() {
        List::<usize>::new();
    }

    #[test]
    fn get_non_existent() {
        let storage = List::<usize>::new();
        assert_eq!(None, storage.get(0));
    }

    #[test]
    fn insert_get() {
        let storage = List::<usize>::new();

        storage.insert(13, 123);
        assert_eq!(Some(&123), storage.get(13));
    }
    #[test]
    fn insert_get_other() {
        let storage = List::<usize>::new();

        storage.insert(13, 123);
        assert_eq!(None, storage.get(14));
    }
}
