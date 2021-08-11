//! TODO

use std::{
    convert::TryInto,
    hash::{Hash, Hasher},
    sync::atomic,
};

struct IDHasher {
    result: u64,
}
impl IDHasher {
    pub fn get_id(thread_id: &std::thread::ThreadId) -> u64 {
        let mut hasher = IDHasher { result: 0 };

        thread_id.hash(&mut hasher);
        hasher.finish()
    }
}
impl std::hash::Hasher for IDHasher {
    fn write(&mut self, bytes: &[u8]) {
        if bytes.len() == 8 {
            self.result = u64::from_le_bytes(bytes.try_into().unwrap());
            return;
        }

        println!("Bytes: {:?}", bytes);
    }
    fn finish(&self) -> u64 {
        self.result
    }
}

struct Entry<T> {
    pub id: u64,
    pub data: T,
    pub next: atomic::AtomicPtr<Self>,
}

/// TODO
pub struct ThreadData<T> {
    head: atomic::AtomicPtr<Entry<T>>,
}

impl<T> ThreadData<T> {
    /// TODO
    pub fn new() -> Self {
        Self {
            head: atomic::AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    fn insert(&self, data: T) -> &T {
        let id = IDHasher::get_id(&std::thread::current().id());
        let new_entry_ptr = Box::into_raw(Box::new(Entry {
            id,
            data,
            next: atomic::AtomicPtr::new(std::ptr::null_mut()),
        }));
        let new_entry = unsafe { &*new_entry_ptr };

        let head_ptr = self.head.load(atomic::Ordering::SeqCst);
        if head_ptr.is_null() {
            if self
                .head
                .compare_exchange(
                    std::ptr::null_mut(),
                    new_entry_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                return &new_entry.data;
            }
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
            }

            current = unsafe { &*next_ptr };
        }
    }

    /// TODO
    pub fn get(&self) -> Option<&T> {
        let thread_id = std::thread::current().id();
        let id = IDHasher::get_id(&thread_id);

        let head_ptr = self.head.load(atomic::Ordering::SeqCst);
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
    pub fn get_or<F>(&self, create: F) -> &T
    where
        F: Fn() -> T,
    {
        match self.get() {
            Some(d) => d,
            None => {
                let data = create();
                self.insert(data)
            }
        }
    }
}

impl<T> Drop for ThreadData<T> {
    fn drop(&mut self) {
        let head_ptr = self.head.load(atomic::Ordering::SeqCst);
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
