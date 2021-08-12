//! # Thread-Local Lock-Free Storage
//! This module provides a Datastructure for Thread-Local Storage that is also
//! lock-free and is therefore useable in other lock-free Datastructures.

mod id;
use std::fmt::Debug;

use id::Id;

mod storage;

/// A Storage-Container for Thread Local Data
pub struct ThreadData<T> {
    storage: storage::Storage<T>,
}

impl<T> Debug for ThreadData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Thread-Data<{}> ()", std::any::type_name::<T>())
    }
}

impl<T> ThreadData<T> {
    /// Creates a new Storage-Container
    pub fn new() -> Self {
        Self {
            storage: storage::Storage::new(),
        }
    }

    /// Attempts to load the stored Data for the current Thread
    pub fn get(&self) -> Option<&T> {
        let id = Id::new().as_u64();

        self.storage.get(id)
    }

    /// Attempts to load the stored for the current Thread or creates + stores
    /// new Data if it does not currently exist
    pub fn get_or<F>(&self, create: F) -> &T
    where
        F: Fn() -> T,
    {
        let id = Id::new().as_u64();
        match self.storage.get(id) {
            Some(d) => d,
            None => {
                let data = create();
                self.storage.insert(id, data)
            }
        }
    }
}

impl<T> Default for ThreadData<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn new() {
        ThreadData::<usize>::new();
    }

    #[test]
    fn get_or_new_item() {
        let data = ThreadData::<usize>::new();

        let result = data.get_or(|| 15);
        assert_eq!(15, *result);
    }

    #[test]
    fn get_or_existing_item() {
        let data = ThreadData::<usize>::new();

        let result = data.get_or(|| 15);
        assert_eq!(15, *result);

        let result = data.get_or(|| 20);
        assert_eq!(15, *result);
    }

    #[test]
    fn get_or_different_threads() {
        let data = Arc::new(ThreadData::<usize>::new());

        let handles: Vec<_> = (0..4)
            .map(|number| {
                let c_data = data.clone();
                std::thread::spawn(move || {
                    let result = c_data.get_or(|| number);
                    assert_eq!(number, *result);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
