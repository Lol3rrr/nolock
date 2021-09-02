use std::sync::atomic;

use crate::allocator::lrmalloc::{descriptor::Descriptor, util::list::List};

pub struct Collection {
    list: List<atomic::AtomicPtr<Descriptor>>,
}

impl Collection {
    pub fn new() -> Self {
        Self {
            list: List::new(atomic::AtomicPtr::new(std::ptr::null_mut())),
        }
    }

    pub fn insert(&self, descriptor: *mut Descriptor) {
        for a_ptr in self.list.iter() {
            if !a_ptr.load(atomic::Ordering::Acquire).is_null() {
                continue;
            }

            if a_ptr
                .compare_exchange(
                    std::ptr::null_mut(),
                    descriptor,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                return;
            }
        }

        self.list.append(atomic::AtomicPtr::new(descriptor));
    }

    pub fn get(&self, ptr: *mut u8) -> Option<*mut Descriptor> {
        for a_ptr in self.list.iter() {
            let desc_ptr = a_ptr.load(atomic::Ordering::Acquire);
            if desc_ptr.is_null() {
                continue;
            }

            let desc = unsafe { &*desc_ptr };

            if desc.contains(ptr) {
                return Some(desc_ptr);
            }
        }

        None
    }

    pub fn remove(&self, descriptor: *mut Descriptor) {
        for a_ptr in self.list.iter() {
            let desc_ptr = a_ptr.load(atomic::Ordering::Acquire);
            if desc_ptr.is_null() {
                continue;
            }

            if desc_ptr == descriptor {
                let _ = a_ptr.compare_exchange(
                    desc_ptr,
                    std::ptr::null_mut(),
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                );
            }
        }
    }
}

unsafe impl Sync for Collection {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_descriptor() {
        let collection = Collection::new();

        collection.insert(0x123 as *mut Descriptor);
    }
}
