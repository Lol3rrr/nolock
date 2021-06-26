use std::{fmt::Debug, mem::ManuallyDrop, sync::atomic};

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
    pub fn boxed(previous: *const BufferList<T>, position_in_queue: usize) -> Box<Self> {
        let buffer = {
            let mut raw = Vec::with_capacity(BUFFER_SIZE);
            for _ in 0..BUFFER_SIZE {
                raw.push(Node::default());
            }

            raw
        };

        Box::new(Self {
            previous,
            next: atomic::AtomicPtr::new(0 as *mut BufferList<T>),
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
    fn fold(&self) -> Option<ManuallyDrop<Box<BufferList<T>>>> {
        let next_ptr = self.next.load(atomic::Ordering::Acquire);
        // This acts as both the check for whether or not this is the End of
        // the Buffers (line 42) as well as the check in line 47
        if next_ptr.is_null() {
            return None;
        }

        let previous_ptr = self.previous;

        let mut next = ManuallyDrop::new(unsafe { Box::from_raw(next_ptr) });
        next.previous = previous_ptr;

        let previous =
            ManuallyDrop::new(unsafe { Box::from_raw(previous_ptr as *mut BufferList<T>) });
        previous.next.store(next_ptr, atomic::Ordering::Release);

        Some(next)
    }

    /// Attempts to find a Set Node starting from `tmp_head`
    ///
    /// # Returns:
    /// This functions returns the Some with the index of a Set node or
    /// returns None if no Set node could be found
    pub fn scan(
        mut tmp_head_of_queue: ManuallyDrop<Box<BufferList<T>>>,
        mut tmp_head: usize,
    ) -> (ManuallyDrop<Box<BufferList<T>>>, Option<usize>) {
        let mut flag_move_to_new_buffer = false;
        let mut flag_buffer_all_handled = true;

        let mut tmp_n = {
            let n_ref = tmp_head_of_queue.buffer.get(tmp_head).unwrap();
            let n_ptr = n_ref as *const Node<T> as *mut Node<T>;
            ManuallyDrop::new(unsafe { Box::from_raw(n_ptr) })
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
                } else {
                    let next_ptr = tmp_head_of_queue.next.load(atomic::Ordering::Acquire);
                    if next_ptr.is_null() {
                        return (tmp_head_of_queue, None);
                    }

                    let next = ManuallyDrop::new(unsafe { Box::from_raw(next_ptr) });

                    tmp_head_of_queue = next;
                    tmp_head = tmp_head_of_queue.head;
                    flag_buffer_all_handled = true;
                    flag_move_to_new_buffer = true;
                }
            } else {
                tmp_n = {
                    let n_ref = tmp_head_of_queue.buffer.get(tmp_head).unwrap();
                    let n_ptr = n_ref as *const Node<T> as *mut Node<T>;
                    ManuallyDrop::new(unsafe { Box::from_raw(n_ptr) })
                };
            }
        }
        (tmp_head_of_queue, Some(tmp_head))
    }

    pub fn rescan(
        head_of_queue_ptr: *mut BufferList<T>,
        mut tempHeadOfQueue: ManuallyDrop<Box<BufferList<T>>>,
        mut tempHead: usize,
    ) -> (ManuallyDrop<Box<BufferList<T>>>, usize) {
        let mut scan_head_of_queue = ManuallyDrop::new(unsafe { Box::from_raw(head_of_queue_ptr) });

        let mut scan_head = scan_head_of_queue.head;
        loop {
            if scan_head_of_queue.position_in_queue == tempHeadOfQueue.position_in_queue
                && scan_head >= tempHead - 1
            {
                break;
            }

            if scan_head >= BUFFER_SIZE {
                let scan_next_ptr = scan_head_of_queue.next.load(atomic::Ordering::Acquire);
                scan_head_of_queue = ManuallyDrop::new(unsafe { Box::from_raw(scan_next_ptr) });
                scan_head = scan_head_of_queue.head;
            }

            let scan_n = scan_head_of_queue.buffer.get(scan_head).unwrap();

            if NodeState::Set == scan_n.get_state() {
                tempHead = scan_head;
                tempHeadOfQueue = scan_head_of_queue;
                scan_head_of_queue = ManuallyDrop::new(unsafe { Box::from_raw(head_of_queue_ptr) });
                scan_head = scan_head_of_queue.head;
            }

            scan_head += 1;
        }

        (tempHeadOfQueue, tempHead)
    }

    /// This attempts to allocate a new BufferList and store it as the next-Ptr for
    /// this Buffer as well as storing it as the new Tail-Of-Queue
    pub fn allocate_next(
        &self,
        self_ptr: *mut BufferList<T>,
        tail_of_queue: &atomic::AtomicPtr<BufferList<T>>,
    ) {
        let next_buffer =
            BufferList::boxed(self_ptr as *const BufferList<T>, self.position_in_queue + 1);
        let next_buffer_ptr = Box::into_raw(next_buffer);

        match self.next.compare_exchange(
            0 as *mut BufferList<T>,
            next_buffer_ptr,
            atomic::Ordering::SeqCst,
            atomic::Ordering::SeqCst,
        ) {
            Ok(_) => {
                match tail_of_queue.compare_exchange(
                    self_ptr,
                    next_buffer_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => {}
                    Err(_) => {}
                };
            }
            Err(_) => {
                drop(unsafe { Box::from_raw(next_buffer_ptr) });
            }
        };
    }
}

impl<T> Debug for BufferList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

    #[test]
    fn folding_success() {
        let tail_ptr = atomic::AtomicPtr::new(0 as *mut BufferList<u32>);

        let first_list = BufferList::boxed(0 as *const BufferList<u32>, 0);
        let first_list_ptr = Box::into_raw(first_list);
        let first_list = unsafe { Box::from_raw(first_list_ptr) };

        first_list.allocate_next(first_list_ptr, &tail_ptr);

        let second_list_ptr = first_list.next.load(atomic::Ordering::SeqCst);
        let second_list = ManuallyDrop::new(unsafe { Box::from_raw(second_list_ptr) });

        second_list.allocate_next(second_list_ptr, &tail_ptr);
        let third_list_ptr = second_list.next.load(atomic::Ordering::SeqCst);
        let third_list = ManuallyDrop::new(unsafe { Box::from_raw(third_list_ptr) });

        let result_next = second_list.fold().unwrap();

        assert_eq!(
            third_list_ptr,
            first_list.next.load(atomic::Ordering::SeqCst)
        );
        assert_eq!(first_list_ptr, third_list.previous as *mut BufferList<u32>);
        assert_eq!(first_list_ptr, result_next.previous as *mut BufferList<u32>);
    }

    #[test]
    fn folding_failure() {
        let tail_ptr = atomic::AtomicPtr::new(0 as *mut BufferList<u32>);

        let first_list = BufferList::boxed(0 as *const BufferList<u32>, 0);
        let first_list_ptr = Box::into_raw(first_list);
        let first_list = unsafe { Box::from_raw(first_list_ptr) };

        first_list.allocate_next(first_list_ptr, &tail_ptr);

        let second_list_ptr = first_list.next.load(atomic::Ordering::SeqCst);
        let second_list = ManuallyDrop::new(unsafe { Box::from_raw(second_list_ptr) });

        assert_eq!(true, second_list.fold().is_none());

        assert_eq!(
            second_list_ptr,
            first_list.next.load(atomic::Ordering::SeqCst)
        );
        assert_eq!(first_list_ptr, second_list.previous as *mut BufferList<u32>);
    }

    #[test]
    fn scan() {
        let raw_list = BufferList::boxed(0 as *const BufferList<u32>, 0);
        let raw_list_ptr = Box::into_raw(raw_list);

        let buffer_list = ManuallyDrop::new(unsafe { Box::from_raw(raw_list_ptr) });
        buffer_list.buffer.get(2).unwrap().store(13);

        let (result_buffer, result_head) =
            BufferList::scan(ManuallyDrop::new(unsafe { Box::from_raw(raw_list_ptr) }), 0);

        assert_eq!(
            buffer_list.position_in_queue,
            result_buffer.position_in_queue
        );
        assert_eq!(Some(2), result_head);
    }
}
