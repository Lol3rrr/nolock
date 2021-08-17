use std::sync::atomic;

use crate::thread_data::StorageBackend;

struct Entry<T> {
    id: u64,
    data: T,
    next: atomic::AtomicPtr<Self>,
}

/// TODO
pub struct List<T> {
    entries: atomic::AtomicPtr<Entry<T>>,
}

impl<T> List<T> {
    /// TODO
    pub fn new() -> Self {
        Self {
            entries: atomic::AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    /// TODO
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

    /// TODO
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

impl<T> StorageBackend<T> for List<T> {
    fn get(&self, id: u64) -> Option<&T> {
        self.get(id)
    }
    fn insert(&self, id: u64, data: T) -> &T {
        self.insert(id, data)
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
