use alloc::{boxed::Box, vec::Vec};
use core::{fmt::Debug, sync::atomic};

use super::{
    node::{Node, NodeState},
    BUFFER_SIZE,
};

/// A single Buffer
pub struct BufferList<T> {
    /// The Previous Buffer in the List of buffers
    pub previous: *const BufferList<T>,
    /// The Next Buffer in the List of buffers
    pub next: atomic::AtomicPtr<BufferList<T>>,
    /// The Buffer of nodes
    pub buffer: Vec<Node<T>>,
    /// The Last read value by the consumer
    pub head: usize,
    /// The Position in the Overall List of Buffers,
    /// initialized to 1
    pub position_in_queue: usize,
}

impl<T> BufferList<T> {
    /// Creates a new Boxed-BufferList
    pub fn boxed(previous: *const Self, position_in_queue: usize) -> Box<Self> {
        let buffer = {
            let mut raw = Vec::with_capacity(BUFFER_SIZE);
            for _ in 0..BUFFER_SIZE {
                raw.push(Node::default());
            }

            raw
        };

        Box::new(Self {
            previous,
            next: atomic::AtomicPtr::new(core::ptr::null_mut()),
            buffer,
            head: 0,
            position_in_queue,
        })
    }

    /// Folds a fully handled buffer in the middle of the queue
    ///
    /// # Behaviour:
    /// Attempts to remove the current BufferList from the overall List of Buffers,
    /// by simply modifying the pointers from the BufferLists around it.
    /// ## Failure-Case:
    /// This operation fails, if the current BufferList does not have a Next-Ptr as
    /// this indicates that the BufferList is the Tail
    ///
    /// # Returns
    /// * `None`: If the current BufferList has no next-Entry.
    /// * `Some(next)`: The Next BufferList, the one following the given BufferList
    #[allow(dead_code)]
    fn fold(&self) -> Option<*mut BufferList<T>> {
        let next_ptr = self.next.load(atomic::Ordering::Acquire);
        // This acts as both the check for whether or not this is the End of
        // the Buffers (line 42) as well as the check in line 47
        if next_ptr.is_null() {
            return None;
        }

        let previous_ptr = self.previous as *mut Self;

        let next = unsafe { &mut *next_ptr };
        next.previous = previous_ptr;

        let previous = unsafe { &*previous_ptr };
        previous.next.store(next_ptr, atomic::Ordering::Release);

        Some(next)
    }

    /// Attempts to find a Set Node starting from `tmp_head`
    ///
    /// # Returns:
    /// This functions returns the Some with the index of a Set node or
    /// returns None if no Set node could be found
    pub fn scan(
        mut tmp_head_of_queue_ptr: *mut BufferList<T>,
        mut tmp_head: usize,
    ) -> (*mut BufferList<T>, Option<usize>) {
        let mut tmp_head_of_queue = unsafe { &*tmp_head_of_queue_ptr };

        let mut flag_move_to_new_buffer = false;
        let mut flag_buffer_all_handled = true;

        let mut tmp_n = {
            let n_ref = tmp_head_of_queue.buffer.get(tmp_head).unwrap();
            let n_ptr = n_ref as *const Node<T> as *mut Node<T>;
            unsafe { &*n_ptr }
        };

        loop {
            let state = tmp_n.get_state();
            if NodeState::Set == state {
                break;
            }

            tmp_head += 1;

            if NodeState::Handled != state {
                flag_buffer_all_handled = false;
            }

            if tmp_head >= BUFFER_SIZE {
                if flag_buffer_all_handled && flag_move_to_new_buffer {
                    /*
                    match tmp_head_of_queue.fold() {
                        Some(n_head_of_queue) => {
                            let old = std::mem::replace(&mut tmp_head_of_queue, n_head_of_queue);
                            drop(ManuallyDrop::into_inner(old));

                            tmp_head = tmp_head_of_queue.head;
                            flag_move_to_new_buffer = true;
                            flag_buffer_all_handled = true;
                        }
                        None => return (tmp_head_of_queue, None),
                    };
                    */
                } else {
                    let next_ptr = tmp_head_of_queue.next.load(atomic::Ordering::Acquire);
                    if next_ptr.is_null() {
                        return (tmp_head_of_queue_ptr, None);
                    }

                    tmp_head_of_queue_ptr = next_ptr;
                    tmp_head_of_queue = unsafe { &*tmp_head_of_queue_ptr };
                    tmp_head = tmp_head_of_queue.head;
                    flag_buffer_all_handled = true;
                    flag_move_to_new_buffer = true;
                }
            } else {
                tmp_n = {
                    let n_ref = tmp_head_of_queue.buffer.get(tmp_head).unwrap();
                    let n_ptr = n_ref as *const Node<T> as *mut Node<T>;
                    unsafe { &*n_ptr }
                };
            }
        }
        (tmp_head_of_queue_ptr, Some(tmp_head))
    }

    /// This attempts to allocate a new BufferList and store it as the next-Ptr for
    /// this Buffer as well as storing it as the new Tail-Of-Queue
    pub fn allocate_next(
        &self,
        self_ptr: *mut Self,
        tail_of_queue: &atomic::AtomicPtr<Self>,
    ) -> *mut Self {
        // Create/Allocate the new Buffer
        let next_buffer = BufferList::boxed(self_ptr as *const Self, self.position_in_queue + 1);
        let next_buffer_ptr = Box::into_raw(next_buffer);

        // Try to append the new Buffer to this one.
        //
        // If this FAILS, that means that another Thread already created a new
        // Buffer and appended it to this one. In that case, we should simply
        // cleanup the new Buffer we created and exit, because essentially
        // the other Thread did the same thing we tried to accomplish so there
        // is nothing for us left to do.
        //
        // If this SUCCEDS, that means we appended our Buffer to the Queue and
        // that we should now also update the Tail of the Queue pointer, as our
        // new Queue is now the latest Element
        match self.next.compare_exchange(
            core::ptr::null_mut(),
            next_buffer_ptr,
            atomic::Ordering::SeqCst,
            atomic::Ordering::Acquire,
        ) {
            Ok(_) => {
                // Attempt to Store our pointer as the Tail of the Queue, as we
                // have now successfully appended the new Buffer to the current
                // Buffer, which was previously the Tail, meaning that the new
                // one is the new Tail
                if tail_of_queue
                    .compare_exchange(
                        self_ptr,
                        next_buffer_ptr,
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    )
                    .is_ok()
                {}

                next_buffer_ptr
            }
            Err(previous) => {
                // Someone else already created the next Buffer following the
                // current one, meaning that we should just clean up the Buffer
                // we created and then we have to do nothing more
                drop(unsafe { Box::from_raw(next_buffer_ptr) });

                previous
            }
        }
    }

    /// Either loads the currently stored Next-Ptr of the Buffer or it will
    /// create a new Buffer and append it to the BufferList
    ///
    /// # Returns
    /// The Ptr to the next Buffer in the BufferList
    pub fn go_to_next(&self, self_ptr: *mut Self, tail: &atomic::AtomicPtr<Self>) -> *mut Self {
        // Load the Ptr to the next Element in the Buffer-List
        let next = self.next.load(atomic::Ordering::Acquire);

        // If the Next-Ptr is not Null, we already have a next Element in the
        // BufferList and can therefore simply return that Element
        if !next.is_null() {
            return next;
        }

        // If we have no next Element in the BufferList, we attempt to create
        // and append a new Buffer
        self.allocate_next(self_ptr, tail)
    }

    /// This function is responsible for deallocating the BufferList pointed to
    /// by the given Ptr, as well as all the previous BufferLists, by walking
    /// the entire Chain of BufferLists
    pub fn deallocate_all(ptr: *mut Self) {
        let mut current_ptr = ptr;
        while !current_ptr.is_null() {
            let current = unsafe { Box::from_raw(current_ptr) };
            current_ptr = current.previous as *mut Self;

            drop(current);
        }
    }
}

impl<T> Debug for BufferList<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "BufferList ( position_in_queue = {}, head = {} )",
            self.position_in_queue, self.head
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::ManuallyDrop;

    #[test]
    fn folding_success() {
        let tail_ptr = atomic::AtomicPtr::new(0 as *mut BufferList<u32>);

        let first_list = BufferList::boxed(0 as *const BufferList<u32>, 0);
        let first_list_ptr = Box::into_raw(first_list);
        let first_list = unsafe { &*first_list_ptr };

        first_list.allocate_next(first_list_ptr, &tail_ptr);

        let second_list_ptr = first_list.next.load(atomic::Ordering::SeqCst);
        let second_list = unsafe { &*second_list_ptr };

        second_list.allocate_next(second_list_ptr, &tail_ptr);
        let third_list_ptr = second_list.next.load(atomic::Ordering::SeqCst);

        let result_next = second_list.fold().unwrap();

        let third_list = unsafe { &*third_list_ptr };

        assert_eq!(
            third_list_ptr,
            first_list.next.load(atomic::Ordering::SeqCst)
        );
        assert_eq!(first_list_ptr, third_list.previous as *mut BufferList<u32>);
        assert_eq!(third_list_ptr, result_next);

        unsafe { Box::from_raw(first_list_ptr) };
        unsafe { Box::from_raw(second_list_ptr) };
        unsafe { Box::from_raw(third_list_ptr) };
    }

    #[test]
    fn folding_failure() {
        let tail_ptr = atomic::AtomicPtr::new(0 as *mut BufferList<u32>);

        let first_list = BufferList::boxed(0 as *const BufferList<u32>, 0);
        let first_list_ptr = Box::into_raw(first_list);
        let first_list = unsafe { Box::from_raw(first_list_ptr) };

        first_list.allocate_next(first_list_ptr, &tail_ptr);

        let second_list_ptr = first_list.next.load(atomic::Ordering::SeqCst);
        let mut second_list = ManuallyDrop::new(unsafe { Box::from_raw(second_list_ptr) });

        assert_eq!(true, second_list.fold().is_none());

        assert_eq!(
            second_list_ptr,
            first_list.next.load(atomic::Ordering::SeqCst)
        );
        assert_eq!(first_list_ptr, second_list.previous as *mut BufferList<u32>);

        unsafe { ManuallyDrop::drop(&mut second_list) };
    }

    #[test]
    fn scan() {
        let raw_list = BufferList::boxed(0 as *const BufferList<u32>, 0);
        let raw_list_ptr = Box::into_raw(raw_list);

        let buffer_list = unsafe { &*raw_list_ptr };
        buffer_list.buffer.get(2).unwrap().store(13);

        let (result_buffer, result_head) = BufferList::scan(raw_list_ptr, 0);

        assert_eq!(raw_list_ptr, result_buffer);
        assert_eq!(Some(2), result_head);

        unsafe { Box::from_raw(raw_list_ptr) };
    }
}
