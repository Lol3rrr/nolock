use std::{alloc::GlobalAlloc, sync::atomic};

use crate::allocator::lrmalloc::descriptor::Descriptor;

struct Node {
    descriptor: atomic::AtomicPtr<Descriptor>,
    next: atomic::AtomicPtr<Self>,
}

impl Node {
    pub fn alloc(self) -> *mut Self {
        let layout = std::alloc::Layout::new::<Self>();
        let block = unsafe { std::alloc::System.alloc(layout) } as *mut Self;
        unsafe { block.write(self) };

        block
    }

    pub fn dealloc(ptr: *mut Self) {
        let layout = std::alloc::Layout::new::<Self>();
        unsafe { std::alloc::System.dealloc(ptr as *mut u8, layout) };
    }
}

/// A Lock-Free Collection of Descriptor-Pointers
///
/// This structure does not provide any garantues about the order of elements
#[derive(Debug)]
pub struct DescriptorCollection {
    head: atomic::AtomicPtr<Node>,
}

impl DescriptorCollection {
    pub const fn new() -> Self {
        Self {
            head: atomic::AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn push(&self, descriptor: *mut Descriptor) {
        let head_ptr = self.head.load(atomic::Ordering::SeqCst);
        if head_ptr.is_null() {
            let new_node_ptr = Node {
                descriptor: atomic::AtomicPtr::new(descriptor),
                next: atomic::AtomicPtr::new(std::ptr::null_mut()),
            }
            .alloc();

            if self
                .head
                .compare_exchange(
                    std::ptr::null_mut(),
                    new_node_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                return;
            }

            Node::dealloc(new_node_ptr);
        }

        let head_ptr = self.head.load(atomic::Ordering::SeqCst);
        let mut current = unsafe { &*head_ptr };

        loop {
            if current.descriptor.load(atomic::Ordering::SeqCst).is_null() {
                if current
                    .descriptor
                    .compare_exchange(
                        std::ptr::null_mut(),
                        descriptor,
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return;
                }
            }

            let next_ptr = current.next.load(atomic::Ordering::SeqCst);
            if next_ptr.is_null() {
                let new_node_ptr = Node {
                    descriptor: atomic::AtomicPtr::new(descriptor),
                    next: atomic::AtomicPtr::new(std::ptr::null_mut()),
                }
                .alloc();

                match current.next.compare_exchange(
                    std::ptr::null_mut(),
                    new_node_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        return;
                    }
                    Err(ptr) => {
                        current = unsafe { &*ptr };
                    }
                };
            } else {
                current = unsafe { &*next_ptr };
            }
        }
    }

    pub fn try_pop(&self) -> Option<*mut Descriptor> {
        let head_ptr = self.head.load(atomic::Ordering::SeqCst);
        if head_ptr.is_null() {
            return None;
        }

        let mut current = unsafe { &*head_ptr };

        loop {
            let descriptor_ptr = current.descriptor.load(atomic::Ordering::SeqCst);
            if !descriptor_ptr.is_null() {
                if current
                    .descriptor
                    .compare_exchange(
                        descriptor_ptr,
                        std::ptr::null_mut(),
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return Some(descriptor_ptr);
                }
            }

            let next_ptr = current.next.load(atomic::Ordering::SeqCst);
            if next_ptr.is_null() {
                return None;
            }

            current = unsafe { &*next_ptr };
        }
    }
}

impl Default for DescriptorCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        DescriptorCollection::new();
    }

    #[test]
    fn push_multiple() {
        let stack = DescriptorCollection::new();

        stack.push(0x123 as *mut Descriptor);
        stack.push(0x234 as *mut Descriptor);
    }

    #[test]
    fn pop_empty() {
        let stack = DescriptorCollection::new();

        assert_eq!(None, stack.try_pop());
    }

    #[test]
    fn push_pop_multiple() {
        let stack = DescriptorCollection::new();

        stack.push(0x123 as *mut Descriptor);

        assert_eq!(Some(0x123 as *mut Descriptor), stack.try_pop());
        assert_eq!(None, stack.try_pop());
    }
}
