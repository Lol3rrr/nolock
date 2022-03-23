use crate::sync::atomic;

use super::{Entry, HashLevel};

use alloc::boxed::Box;
use core::mem::ManuallyDrop;

pub(crate) struct TargetPtr<K, V>(atomic::AtomicPtr<Entry<K, V>>);

pub(crate) enum PtrType {
    Entry(*mut ()),
    HashLevel(*mut ()),
}

pub(crate) enum LoadResult<'r, K, V, const B: u8> {
    Entry {
        entry: &'r Entry<K, V>,
    },
    HashLevel {
        level: &'r HashLevel<K, V, B>,
        ptr: *mut HashLevel<K, V, B>,
    },
}

impl<K, V> TargetPtr<K, V> {
    pub fn new_hashlevel<const B: u8>(ptr: *mut HashLevel<K, V, B>) -> Self {
        let marked = mark_as_previous(ptr as *const u8) as *mut Entry<K, V>;
        Self(atomic::AtomicPtr::new(marked))
    }

    pub fn raw_load(&self, order: atomic::Ordering) -> *mut () {
        self.0.load(order) as *mut ()
    }

    pub fn load_ptr(&self, order: atomic::Ordering) -> PtrType {
        let ptr = self.0.load(order);
        if is_entry(ptr as *const u8) {
            PtrType::Entry(to_actual_ptr(ptr as *const u8) as *mut ())
        } else {
            PtrType::HashLevel(to_actual_ptr(ptr as *const u8) as *mut ())
        }
    }

    pub fn load<const B: u8>(&self) -> LoadResult<'_, K, V, B> {
        let ptr = self.0.load(atomic::Ordering::SeqCst);
        if is_entry(ptr as *const u8) {
            let ptr = to_actual_ptr(ptr as *const u8) as *const ();
            let ptr = ptr as *mut Entry<K, V>;

            LoadResult::Entry {
                entry: unsafe { &*ptr },
            }
        } else {
            let ptr = to_actual_ptr(ptr as *const u8) as *const ();
            let ptr = ptr as *mut HashLevel<K, V, B>;

            LoadResult::HashLevel {
                level: unsafe { &*ptr },
                ptr,
            }
        }
    }

    pub fn raw_store(&self, new: *mut (), order: atomic::Ordering) {
        self.0.store(new as *mut Entry<K, V>, order)
    }

    pub fn store_hashlevel(&self, ptr: *mut (), order: atomic::Ordering) {
        let marked = mark_as_previous(ptr as *const u8) as *mut Entry<K, V>;
        self.0.store(marked, order);
    }

    pub fn cas_hashlevel<const B: u8>(
        &self,
        current: *mut Entry<K, V>,
        new: *mut (),
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<*mut Entry<K, V>, *mut Entry<K, V>> {
        let marked = mark_as_previous(new as *const u8) as *mut Entry<K, V>;
        self.0.compare_exchange(current, marked, success, failure)
    }
    pub fn cas_entry<const B: u8>(
        &self,
        current: *mut Entry<K, V>,
        new: *mut (),
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<*mut Entry<K, V>, *mut Entry<K, V>> {
        let marked = mark_as_entry(new as *const u8) as *mut Entry<K, V>;
        self.0.compare_exchange(current, marked, success, failure)
    }
}

pub(crate) fn boxed_hashlevel<K, V, const B: u8>(
    ptr: *mut HashLevel<K, V, B>,
) -> ManuallyDrop<Box<HashLevel<K, V, B>>> {
    let inner = unsafe { Box::from_raw(ptr) };
    ManuallyDrop::new(inner)
}
pub(crate) fn boxed_entry<K, V>(ptr: *mut Entry<K, V>) -> ManuallyDrop<Box<Entry<K, V>>> {
    let boxed = unsafe { Box::from_raw(ptr) };
    ManuallyDrop::new(boxed)
}

pub fn is_entry(ptr: *const u8) -> bool {
    (ptr as usize) & 0x1 == 0
}

pub fn mark_as_previous(ptr: *const u8) -> *const u8 {
    ((ptr as usize) | 0x1) as *const u8
}
pub fn mark_as_entry(ptr: *const u8) -> *const u8 {
    ((ptr as usize) & (usize::MAX - 1)) as *const u8
}

pub fn to_actual_ptr(ptr: *const u8) -> *const u8 {
    ((ptr as usize) & (usize::MAX - 1)) as *const u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_checks() {
        assert_eq!(true, is_entry(0xFFFFFFFFFFFFFFF0 as *const u8));
        assert_eq!(false, is_entry(0xFFFFFFFFFFFFFFF1 as *const u8));
    }

    #[test]
    fn mark_ptrs() {
        assert_eq!(
            0xFFFFFFFFFFFFFFF1 as *const u8,
            mark_as_previous(0xFFFFFFFFFFFFFFF0 as *const u8)
        );
        assert_eq!(
            0xFFFFFFFFFFFFFFF0 as *const u8,
            mark_as_entry(0xFFFFFFFFFFFFFFF0 as *const u8)
        );
    }

    #[test]
    fn to_original() {
        assert_eq!(
            0xFFFFFFFFFFFFFFF0 as *const u8,
            to_actual_ptr(0xFFFFFFFFFFFFFFF1 as *const u8)
        );
        assert_eq!(
            0xFFFFFFFFFFFFFFF0 as *const u8,
            to_actual_ptr(0xFFFFFFFFFFFFFFF0 as *const u8)
        );
    }
}
