use std::{alloc::GlobalAlloc, sync::atomic};

use crate::allocator::lrmalloc::descriptor::Descriptor;

struct Node {
    descriptor: *mut Descriptor,
    next: atomic::AtomicPtr<Self>,
}

impl Node {
    /// Creates a new Node and allocates it using the System-Allocator
    ///
    /// The created node does not have a next Node set
    pub fn alloc_new(descriptor: *mut Descriptor) -> *mut Self {
        let node = Self {
            descriptor,
            next: atomic::AtomicPtr::new(std::ptr::null_mut()),
        };

        let layout = std::alloc::Layout::new::<Self>();
        let raw_ptr = unsafe { std::alloc::System.alloc(layout) } as *mut Self;

        unsafe { raw_ptr.write(node) };

        raw_ptr
    }
}

pub struct Collection {
    head: *mut Node,
}

impl Collection {
    pub fn new() -> Self {
        Self {
            head: Node::alloc_new(std::ptr::null_mut()),
        }
    }

    pub fn insert(&self, descriptor: *mut Descriptor) {
        let mut current = unsafe { &*self.head };

        let new_node = Node::alloc_new(descriptor);

        loop {
            let next_ptr = current.next.load(atomic::Ordering::Acquire);

            if next_ptr.is_null() {
                match current.next.compare_exchange(
                    std::ptr::null_mut(),
                    new_node,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Acquire,
                ) {
                    Ok(_) => return,
                    Err(ptr) => {
                        current = unsafe { &*ptr };
                    }
                };
            } else {
                current = unsafe { &*next_ptr };
            }
        }
    }

    pub fn get(&self, ptr: *mut u8) -> Option<*mut Descriptor> {
        let mut current = unsafe { &*self.head };

        loop {
            let desc_ptr = current.descriptor;
            if !desc_ptr.is_null() {
                let desc = unsafe { &*desc_ptr };
                if desc.contains(ptr) {
                    return Some(desc_ptr);
                }
            }

            let next_ptr = current.next.load(atomic::Ordering::Acquire);
            if next_ptr.is_null() {
                return None;
            }

            current = unsafe { &*next_ptr };
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
