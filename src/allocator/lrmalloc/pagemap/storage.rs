use std::{alloc::GlobalAlloc, sync::atomic};

use crate::allocator::lrmalloc::descriptor::Descriptor;

struct Node {
    descriptor: *mut Descriptor,
    next: atomic::AtomicPtr<Self>,
}

impl Node {
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
            let next_ptr = current.next.load(atomic::Ordering::AcqRel);

            if next_ptr.is_null() {
                match current.next.compare_exchange(
                    std::ptr::null_mut(),
                    new_node,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::AcqRel,
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
}

unsafe impl Sync for Collection {}
