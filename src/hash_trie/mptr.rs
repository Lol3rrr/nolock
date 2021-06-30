use crate::hazard_ptr;

use super::{Entry, HashLevel};
use std::{mem::ManuallyDrop, sync::atomic};

pub(crate) struct TargetPtr<K, V>(atomic::AtomicPtr<Entry<K, V>>);

impl<K, V> TargetPtr<K, V> {
    pub fn new_entry(ptr: *mut Entry<K, V>) -> Self {
        let marked = mark_as_entry(ptr as *const u8) as *mut Entry<K, V>;
        Self(atomic::AtomicPtr::new(marked))
    }
    pub fn new_hashlevel<const B: u8>(ptr: *mut HashLevel<K, V, B>) -> Self {
        let marked = mark_as_previous(ptr as *const u8) as *mut Entry<K, V>;
        Self(atomic::AtomicPtr::new(marked))
    }

    pub fn load<const B: u8>(
        &self,
        tmp_guard: &mut hazard_ptr::Guard<Entry<K, V>>,
    ) -> Option<(
        ManuallyDrop<Box<HashLevel<K, V, B>>>,
        *mut HashLevel<K, V, B>,
    )> {
        tmp_guard.protect(&self.0, atomic::Ordering::SeqCst);
        if is_entry(tmp_guard.raw() as *const u8) {
            None
        } else {
            let ptr = to_actual_ptr(tmp_guard.raw() as *const u8) as *const ();
            let ptr = ptr as *mut HashLevel<K, V, B>;
            Some((ManuallyDrop::new(unsafe { Box::from_raw(ptr) }), ptr))
        }
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

pub(crate) enum PtrTarget<K, V, const B: u8> {
    HashLevel(*mut HashLevel<K, V, B>),
    Entry(*mut Entry<K, V>),
}

pub(crate) fn boxed_hashlevel<K, V, const B: u8>(
    ptr: *mut HashLevel<K, V, B>,
) -> ManuallyDrop<Box<HashLevel<K, V, B>>> {
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

    pub fn load<K, V, const B: u8>(&self, order: atomic::Ordering) -> PtrTarget<K, V, B> {
        let raw = self.ptr.load(order);

        if is_entry(raw as *const u8) {
            PtrTarget::Entry(to_actual_ptr(raw as *const u8) as *mut Entry<K, V>)
        } else {
            PtrTarget::HashLevel(to_actual_ptr(raw as *const u8) as *mut HashLevel<K, V, B>)
        }
    }
    pub fn store_hashlevel<K, V, const B: u8>(
        &self,
        ptr: *mut HashLevel<K, V, B>,
        order: atomic::Ordering,
    ) {
        let marked = mark_as_previous(ptr as *const u8) as *mut u8;
        self.ptr.store(marked, order);
    }
    pub fn store_entry<K, V>(&self, ptr: *mut Entry<K, V>, order: atomic::Ordering) {
        let marked = mark_as_entry(ptr as *const u8) as *mut u8;
        self.ptr.store(marked, order);
    }

    pub fn cas_hashlevel<K, V, const B: u8>(
        &self,
        current: *mut u8,
        new: *mut HashLevel<K, V, B>,
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
