use std::{alloc::GlobalAlloc, sync::atomic};

struct Node<T> {
    data: T,
    next: atomic::AtomicPtr<Self>,
}

impl<T> Node<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data,
            next: atomic::AtomicPtr::new(std::ptr::null_mut()),
        }
    }

    pub fn alloc<G>(self, allocator: &G) -> *mut Self
    where
        G: GlobalAlloc,
    {
        let layout = std::alloc::Layout::new::<Self>();
        let raw_ptr = unsafe { allocator.alloc(layout) };
        let ptr: *mut Self = raw_ptr as *mut Self;

        unsafe { ptr.write(self) };

        ptr
    }

    pub fn dealloc<G>(ptr: *mut Self, allocator: &G)
    where
        G: GlobalAlloc,
    {
        let layout = std::alloc::Layout::new::<Self>();
        unsafe { allocator.dealloc(ptr as *mut u8, layout) };
    }
}

struct NodeIter<T> {
    current: *mut Node<T>,
}

impl<T> Iterator for NodeIter<T> {
    type Item = *mut Node<T>;

    fn next(&mut self) -> Option<Self::Item> {
        let current_ptr = self.current;
        if current_ptr.is_null() {
            return None;
        }

        let current = unsafe { &*current_ptr };
        let new_ptr = current.next.load(atomic::Ordering::Acquire);

        self.current = new_ptr;

        Some(current_ptr)
    }
}

pub struct List<T> {
    head: Node<T>,
}

impl<T> List<T> {
    pub const fn new(initial_entry: T) -> Self {
        let initial_node = Node::new(initial_entry);

        Self { head: initial_node }
    }

    /// # Safety:
    /// The Caller must ensure that the List is never moved while the given
    /// Ptr is still in use
    unsafe fn head_ptr(&self) -> *mut Node<T> {
        let field_ref = &self.head;
        field_ref as *const Node<T> as *mut Node<T>
    }

    fn node_iter(&self) -> NodeIter<T> {
        let head_ptr = unsafe { self.head_ptr() };
        NodeIter { current: head_ptr }
    }

    pub fn append(&self, data: T) {
        let new_node = Node::new(data);
        let new_node_ptr = new_node.alloc(&std::alloc::System);

        let mut iter = self.node_iter().peekable();

        let mut latest = &self.head;
        while iter.peek().is_some() {
            let ptr = iter
                .next()
                .expect("We just peeked on the Iterator and found an element so this must succeed");
            latest = unsafe { &*ptr };
        }

        loop {
            match latest.next.compare_exchange(
                std::ptr::null_mut(),
                new_node_ptr,
                atomic::Ordering::AcqRel,
                atomic::Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(ptr) => {
                    latest = unsafe { &*ptr };
                }
            };
        }
    }

    pub fn iter(&self) -> ListIter<'_, T> {
        ListIter {
            node_iter: self.node_iter(),
            _marker: std::marker::PhantomData {},
        }
    }
}

unsafe impl<T> Sync for List<T> {}

impl<T> Drop for List<T> {
    fn drop(&mut self) {
        let mut iter = self.node_iter();
        // Skip the first Element of the Iterator as that is the root instance
        // which was not actually allocated
        let _ = iter.next();

        for node_ptr in iter {
            Node::dealloc(node_ptr, &std::alloc::System);
        }
    }
}

pub struct ListIter<'iter, T> {
    node_iter: NodeIter<T>,
    _marker: std::marker::PhantomData<&'iter ()>,
}

impl<'iter, T> Iterator for ListIter<'iter, T>
where
    T: 'iter,
{
    type Item = &'iter T;

    fn next(&mut self) -> Option<Self::Item> {
        let node_ptr = self.node_iter.next()?;
        let node = unsafe { &*node_ptr };
        Some(&node.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        List::<u8>::new(0);
    }

    #[test]
    fn append_iter() {
        let list = List::<u8>::new(0);

        list.append(123);
        list.append(234);

        let mut iter = list.iter();

        assert_eq!(Some(&0), iter.next());
        assert_eq!(Some(&123), iter.next());
        assert_eq!(Some(&234), iter.next());
        assert_eq!(None, iter.next());
    }

    #[test]
    fn iter_append_middle() {
        let list = List::<usize>::new(0);

        list.append(123);
        list.append(234);

        let mut iter = list.iter();

        assert_eq!(Some(&0), iter.next());
        assert_eq!(Some(&123), iter.next());

        list.append(345);

        assert_eq!(Some(&234), iter.next());
        assert_eq!(Some(&345), iter.next());
        assert_eq!(None, iter.next());
    }
}
