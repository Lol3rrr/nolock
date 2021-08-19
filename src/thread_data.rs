//! # Thread-Local Lock-Free Storage
//! This module provides a Datastructure for Thread-Local Storage that is also
//! lock-free and is therefore useable in other lock-free Datastructures.

mod id;
use std::fmt::Debug;

use id::Id;

pub mod storage;

/// The General Interface used by the [`ThreadDataStorage`] to interface with
/// any sort of Datastructure used to actually store the Data for each
/// individuel Thread.
pub trait StorageBackend<T> {
    /// This should attempt to the Load the Data for the given ID
    fn get(&self, id: u64) -> Option<&T>;
    /// This should create a new Entry with the given ID and Data, which will
    /// later be loaded again. This should then also return a reference to the
    /// Data that can the be used to access it directly.
    ///
    /// # Note
    /// This function will only be called with new ID's and should therefore
    /// never cause an ID collision in the underlying Storage
    fn insert(&self, id: u64, data: T) -> &T;
}

/// A Storage-Container for Thread Local Data
pub struct ThreadDataStorage<S, T> {
    storage: S,
    _marker: std::marker::PhantomData<T>,
}

impl<S, T> Debug for ThreadDataStorage<S, T>
where
    S: StorageBackend<T>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Thread-Data<{}> ()", std::any::type_name::<T>())
    }
}

impl<T> ThreadDataStorage<storage::Trie<T>, T> {
    /// Creates a new Instance using the [`Trie`](storage::Trie) StorageBackend
    pub fn new() -> Self {
        Self::new_storage(storage::Trie::new())
    }
}
impl<T> ThreadDataStorage<storage::List<T>, T> {
    /// Creates a new Instance using the [`List`](storage::List) StorageBackend
    pub fn new() -> Self {
        Self::new_storage(storage::List::new())
    }
}

impl<S, T> ThreadDataStorage<S, T>
where
    S: StorageBackend<T>,
{
    /// Creates a new Instance which uses the given Storage-Backend for all the
    /// Data.
    ///
    /// # Use Case
    /// This should only really be used if you want to use a custom StorageBackend
    /// with it.
    /// Otherwise you should just use [`ThreadData::<T>::new()`] to create a
    /// ThreadDataStorage instance with the Trie StorageBackend
    pub fn new_storage(storage: S) -> Self {
        Self {
            storage,
            _marker: std::marker::PhantomData::default(),
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

impl<T> Default for ThreadDataStorage<storage::Trie<T>, T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T> Default for ThreadDataStorage<storage::List<T>, T> {
    fn default() -> Self {
        Self::new()
    }
}

unsafe impl<S, T> Sync for ThreadDataStorage<S, T> {}
unsafe impl<S, T> Send for ThreadDataStorage<S, T> {}

/// The Default ThreadData Storage with the [`Trie`](storage::Trie) backend.
/// This should be the right fit for basically all Use-Cases as it is the
/// fastest Storage-Backend while also having low memory overhead
pub type ThreadData<T> = ThreadDataStorage<storage::Trie<T>, T>;

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
