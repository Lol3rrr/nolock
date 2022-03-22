use crate::sync::atomic;
use core::{marker::PhantomData, mem::MaybeUninit};

use alloc::boxed::Box;

pub struct BatchList<const N: usize> {
    head: atomic::AtomicPtr<ListEntry<N>>,
}

struct ListEntry<const N: usize> {
    used: atomic::AtomicU8,
    index: usize,
    nodes: [MaybeUninit<*const ()>; N],
    next: atomic::AtomicPtr<ListEntry<N>>,
}

pub struct BatchHandle<'b> {
    used: &'static atomic::AtomicU8,
    index: &'static mut usize,
    nodes: &'static mut [MaybeUninit<*const ()>],
    _marker: PhantomData<&'b ()>,
}

pub struct BatchDrainIterator<'a, const N: usize> {
    current: *mut ListEntry<N>,
    _marker: PhantomData<&'a ()>,
}

impl<const N: usize> BatchList<N> {
    pub fn new() -> Self {
        Self {
            head: atomic::AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    pub fn get_batch(&self) -> BatchHandle<'_> {
        let mut current_ptr = self.head.load(atomic::Ordering::SeqCst);
        while !current_ptr.is_null() {
            let node = unsafe { &*current_ptr };

            if node
                .used
                .compare_exchange(0, 1, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst)
                .is_ok()
            {
                let node = unsafe { &mut *current_ptr };
                return BatchHandle {
                    used: &node.used,
                    index: &mut node.index,
                    nodes: &mut node.nodes,
                    _marker: PhantomData {},
                };
            }

            current_ptr = node.next.load(atomic::Ordering::SeqCst);
        }

        let n_entry = ListEntry::<N>::new();
        n_entry.used.store(1, atomic::Ordering::SeqCst);

        let n_entry_ptr = Box::into_raw(Box::new(n_entry));

        let mut atom_ptr = &self.head;
        loop {
            match atom_ptr.compare_exchange(
                core::ptr::null_mut(),
                n_entry_ptr,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
            ) {
                Ok(_) => break,
                Err(other_ptr) => {
                    let node = unsafe { &*other_ptr };
                    atom_ptr = &node.next;
                }
            };
        }

        let entry_ref = unsafe { &mut *n_entry_ptr };

        BatchHandle {
            used: &entry_ref.used,
            index: &mut entry_ref.index,
            nodes: &mut entry_ref.nodes,
            _marker: PhantomData {},
        }
    }

    pub fn drain(&mut self) -> BatchDrainIterator<'_, N> {
        BatchDrainIterator {
            current: self.head.load(atomic::Ordering::SeqCst),
            _marker: PhantomData {},
        }
    }
}

impl<const N: usize> Drop for BatchList<N> {
    fn drop(&mut self) {
        let mut ptr = self.head.load(atomic::Ordering::SeqCst);

        while !ptr.is_null() {
            let node = unsafe { &*ptr };

            let next_ptr = node.next.load(atomic::Ordering::SeqCst);
            let _ = unsafe { Box::from_raw(ptr) };

            ptr = next_ptr;
        }
    }
}

impl<const N: usize> ListEntry<N> {
    pub fn new() -> Self {
        Self {
            used: atomic::AtomicU8::new(0),
            index: 0,
            nodes: [MaybeUninit::uninit(); N],
            next: atomic::AtomicPtr::new(core::ptr::null_mut()),
        }
    }
}

impl<'b> BatchHandle<'b> {
    pub fn try_retire(&mut self, ptr: *const ()) -> Result<(), *const ()> {
        if *self.index == self.nodes.len() {
            return Err(ptr);
        }

        *(self.nodes.get_mut(*self.index).unwrap()) = MaybeUninit::new(ptr);
        *self.index += 1;

        Ok(())
    }

    pub fn batch_iter(&mut self) -> impl Iterator<Item = *const ()> + '_ {
        let length = *self.index;
        *self.index = 0;

        self.nodes.iter_mut().take(length).map(|node| {
            let value = unsafe { (*node).assume_init() };
            *node = MaybeUninit::uninit();
            value
        })
    }
}
impl<'b> Drop for BatchHandle<'b> {
    fn drop(&mut self) {
        self.used.store(0, atomic::Ordering::SeqCst);
    }
}

impl<'b, const N: usize> Iterator for BatchDrainIterator<'b, N> {
    type Item = BatchHandle<'b>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.is_null() {
            return None;
        }

        let current = unsafe { &mut *self.current };
        let next = current.next.load(atomic::Ordering::SeqCst);
        self.current = next;

        Some(BatchHandle {
            used: &current.used,
            nodes: &mut current.nodes,
            index: &mut current.index,
            _marker: PhantomData {},
        })
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    use super::*;

    #[test]
    fn create_empty() {
        let list = BatchList::<2>::new();
        drop(list);
    }

    #[test]
    fn empty_list_get_batch() {
        let list = BatchList::<2>::new();

        let handle = list.get_batch();
        drop(handle);
    }

    #[test]
    fn reuse_handle() {
        let list = BatchList::<2>::new();

        let handle = list.get_batch();
        drop(handle);

        let handle = list.get_batch();
        drop(handle);
    }

    #[test]
    fn multiple_handles() {
        let list = BatchList::<2>::new();

        let handle1 = list.get_batch();

        let handle2 = list.get_batch();

        drop(handle1);
        drop(handle2);
    }

    #[test]
    fn retire_empty() {
        let list = BatchList::<2>::new();

        let mut handle = list.get_batch();
        handle.try_retire(core::ptr::null_mut()).unwrap();
        handle.try_retire(core::ptr::null_mut()).unwrap();
        handle.try_retire(core::ptr::null_mut()).unwrap_err();
    }

    #[test]
    fn retire_iter() {
        let list = BatchList::<2>::new();

        let mut handle = list.get_batch();
        handle.try_retire(core::ptr::null_mut()).unwrap();
        handle.try_retire(core::ptr::null_mut()).unwrap();
        handle.try_retire(core::ptr::null_mut()).unwrap_err();

        let mut batch_iter = handle.batch_iter();
        assert_eq!(Some(core::ptr::null()), batch_iter.next());
        assert_eq!(Some(core::ptr::null()), batch_iter.next());
        assert_eq!(None, batch_iter.next());
        drop(batch_iter);

        handle.try_retire(core::ptr::null_mut()).unwrap();
        handle.try_retire(core::ptr::null_mut()).unwrap();
        handle.try_retire(core::ptr::null_mut()).unwrap_err();
    }
}

#[cfg(all(test, loom))]
mod loom_tests {
    use super::*;
    use loom::sync::Arc;
    use loom::thread;

    #[test]
    fn concurrent_gets() {
        loom::model(|| {
            let list = Arc::new(BatchList::<2>::new());

            let l1 = list.clone();
            let l2 = list.clone();

            let handle1 = thread::spawn(move || {
                let mut batch = l1.get_batch();
                batch.try_retire(core::ptr::null()).unwrap();
            });

            let handle2 = thread::spawn(move || {
                let mut batch = l2.get_batch();
                batch.try_retire(core::ptr::null()).unwrap();
            });

            handle1.join();
            handle2.join();
        });
    }
}
