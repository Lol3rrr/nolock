use std::{fmt::Debug, sync::atomic};

use crate::allocator::lrmalloc::{descriptor::Descriptor, util::list::List};

pub struct Collection {
    list: List<atomic::AtomicPtr<Descriptor>>,
}

impl Collection {
    pub const fn new() -> Self {
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

impl Debug for Collection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;

        for a_ptr in self.list.iter() {
            let ptr = a_ptr.load(atomic::Ordering::SeqCst);
            if ptr.is_null() {
                write!(f, "{:p},", ptr)?;
            } else {
                let desc = unsafe { &*ptr };
                write!(f, "{:?},", desc)?;
            }
        }

        write!(f, "]")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_descriptor() {
        let collection = Collection::new();

        collection.insert(0x123 as *mut Descriptor);
    }

    #[test]
    fn insert_get() {
        let collection = Collection::new();

        let desc_ptr = Box::into_raw(Box::new(Descriptor::new(
            128,
            4,
            Some(1),
            0x1000 as *mut u8,
        )));

        collection.insert(desc_ptr);

        let result = collection.get(0x1000 as *mut u8);
        assert_eq!(Some(desc_ptr), result);
    }
}
