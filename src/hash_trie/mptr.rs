use super::{Entry, HashLevel};
use std::{mem::ManuallyDrop, sync::atomic};

pub(crate) enum PtrTarget<K, V> {
    HashLevel(*mut HashLevel<K, V>),
    Entry(*mut Entry<K, V>),
}

pub(crate) fn boxed_hashlevel<K, V>(
    ptr: *mut HashLevel<K, V>,
) -> ManuallyDrop<Box<HashLevel<K, V>>> {
    let boxed = unsafe { Box::from_raw(ptr) };
    ManuallyDrop::new(boxed)
}
pub(crate) fn boxed_entry<K, V>(ptr: *mut Entry<K, V>) -> ManuallyDrop<Box<Entry<K, V>>> {
    let boxed = unsafe { Box::from_raw(ptr) };
    ManuallyDrop::new(boxed)
}

pub(crate) struct Atomic {
    ptr: atomic::AtomicPtr<u8>,
}

impl Atomic {
    pub fn new<T>(ptr: *mut T) -> Self {
        Self {
            ptr: atomic::AtomicPtr::new(ptr as *mut u8),
        }
    }

    pub fn load<K, V>(&self, order: atomic::Ordering) -> PtrTarget<K, V> {
        let raw = self.ptr.load(order);

        if is_entry(raw as *const u8) {
            PtrTarget::Entry(to_actual_ptr(raw as *const u8) as *mut Entry<K, V>)
        } else {
            PtrTarget::HashLevel(to_actual_ptr(raw as *const u8) as *mut HashLevel<K, V>)
        }
    }
    pub fn store_hashlevel<K, V>(&self, ptr: *mut HashLevel<K, V>, order: atomic::Ordering) {
        let marked = mark_as_previous(ptr as *const u8) as *mut u8;
        self.ptr.store(marked, order);
    }
    pub fn store_entry<K, V>(&self, ptr: *mut Entry<K, V>, order: atomic::Ordering) {
        let marked = mark_as_entry(ptr as *const u8) as *mut u8;
        self.ptr.store(marked, order);
    }

    pub fn cas_hashlevel<K, V>(
        &self,
        current: *mut u8,
        new: *mut HashLevel<K, V>,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<*mut u8, *mut u8> {
        let marked = mark_as_previous(new as *const u8) as *mut u8;
        self.ptr.compare_exchange(current, marked, success, failure)
    }
    pub fn cas_entry<K, V>(
        &self,
        current: *mut u8,
        new: *mut Entry<K, V>,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<*mut u8, *mut u8> {
        let marked = mark_as_entry(new as *const u8) as *mut u8;
        self.ptr.compare_exchange(current, marked, success, failure)
    }
}

pub fn is_previous(ptr: *const u8) -> bool {
    (ptr as usize) & 0x1 == 1
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
        assert_eq!(true, is_previous(0xFFFFFFFFFFFFFFF1 as *const u8));
        assert_eq!(false, is_previous(0xFFFFFFFFFFFFFFF0 as *const u8));
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
