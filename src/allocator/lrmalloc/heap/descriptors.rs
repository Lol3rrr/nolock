use std::sync::atomic;

use crate::allocator::lrmalloc::{descriptor::Descriptor, util};

pub struct RecycleList {
    list: util::list::List<atomic::AtomicPtr<Descriptor>>,
}

impl RecycleList {
    pub fn new() -> Self {
        Self {
            list: util::list::List::new(atomic::AtomicPtr::new(std::ptr::null_mut())),
        }
    }

    pub fn add_descriptor(&self, desc: *mut Descriptor) {
        for item in self.list.iter() {
            if !item.load(atomic::Ordering::Acquire).is_null() {
                continue;
            }

            if item
                .compare_exchange(
                    std::ptr::null_mut(),
                    desc,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                return;
            }
        }

        self.list.append(atomic::AtomicPtr::new(desc));
    }

    pub fn get_descriptor(&self) -> Option<*mut Descriptor> {
        for item in self.list.iter() {
            let ptr = item.load(atomic::Ordering::Acquire);
            if ptr.is_null() {
                continue;
            }

            if item
                .compare_exchange(
                    ptr,
                    std::ptr::null_mut(),
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                return Some(ptr);
            }
        }

        None
    }
}
