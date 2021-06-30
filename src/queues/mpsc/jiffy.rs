//! The implemenation of a Lock-Free, possibly Wait-Free, MPSC Queue
//!
//! # Reference:
//! * [Jiffy: A Fast, Memory Efficient, Wait-Free Multi-Producers Single-Consumer Queue](https://arxiv.org/pdf/2010.14189.pdf)

use std::{
    fmt::Debug,
    mem::ManuallyDrop,
    sync::{atomic, Arc},
};

const BUFFER_SIZE: usize = 256;

mod node;
use node::NodeState;

mod bufferlist;
use bufferlist::BufferList;

/// One of the Senders
pub struct Sender<T> {
    tail: Arc<atomic::AtomicUsize>,
    tail_of_queue: Arc<atomic::AtomicPtr<BufferList<T>>>,
}
/// The Single Receiver
pub struct Receiver<T> {
    head_of_queue: *const BufferList<T>,
}

impl<T> Sender<T> {
    /// Enqueues the given piece of Data
    pub fn enqueue(&self, data: T) {
        let location = self.tail.fetch_add(1, atomic::Ordering::SeqCst);

        let mut tmp_buffer_ptr = self.tail_of_queue.load(atomic::Ordering::Acquire);
        let mut tmp_buffer = ManuallyDrop::new(unsafe { Box::from_raw(tmp_buffer_ptr) });

        let mut end = tmp_buffer.position_in_queue * BUFFER_SIZE;
        while location >= end {
            if tmp_buffer.next.load(atomic::Ordering::Acquire).is_null() {
                tmp_buffer.allocate_next(tmp_buffer_ptr, &self.tail_of_queue);
            }

            tmp_buffer_ptr = self.tail_of_queue.load(atomic::Ordering::Acquire);
            tmp_buffer = ManuallyDrop::new(unsafe { Box::from_raw(tmp_buffer_ptr) });

            end = tmp_buffer.position_in_queue * BUFFER_SIZE;
        }

        let mut start = (tmp_buffer.position_in_queue - 1) * BUFFER_SIZE;
        while location < start {
            tmp_buffer_ptr = tmp_buffer.previous as *mut BufferList<T>;
            tmp_buffer = ManuallyDrop::new(unsafe { Box::from_raw(tmp_buffer_ptr) });

            start = (tmp_buffer.position_in_queue - 1) * BUFFER_SIZE;
        }

        let index = location - start;

        let node = tmp_buffer.buffer.get(index).unwrap();
        node.store(data);

        // TODO
        // Possible optimization regarding to pre-allocate the next buffer early
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        Self {
            tail: self.tail.clone(),
            tail_of_queue: self.tail_of_queue.clone(),
        }
    }
}

impl<T> Debug for Sender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sender ()")
    }
}

impl<T> Receiver<T> {
    fn load_head_of_queue(&self) -> ManuallyDrop<Box<BufferList<T>>> {
        let ptr = self.head_of_queue;
        ManuallyDrop::new(unsafe { Box::from_raw(ptr as *mut BufferList<T>) })
    }

    fn move_to_next_buffer(&mut self) -> bool {
        let current_queue = self.load_head_of_queue();

        if current_queue.head >= BUFFER_SIZE {
            // Lines 63 - 65
            // can be ommited in this case as the next_ptr will then also be 0 and therefore
            // the next check should catch that

            let next_ptr = current_queue.next.load(atomic::Ordering::SeqCst);
            if next_ptr.is_null() {
                return false;
            }

            let previous = unsafe { Box::from_raw(self.head_of_queue as *mut BufferList<T>) };

            self.head_of_queue = next_ptr;

            drop(previous);
        }

        true
    }

    /// Attempts to dequeue the next entry in the Queue
    pub fn dequeue(&mut self) -> Option<T> {
        let mut current_queue = self.load_head_of_queue();

        let mut n = match current_queue.buffer.get(current_queue.head) {
            Some(n) => n,
            None => {
                self.move_to_next_buffer();
                current_queue = self.load_head_of_queue();
                current_queue.buffer.get(current_queue.head)?
            }
        };
        while n.get_state() == NodeState::Handled {
            current_queue.head += 1;

            if !self.move_to_next_buffer() {
                return None;
            }

            current_queue = self.load_head_of_queue();
            n = match current_queue.buffer.get(current_queue.head) {
                Some(n) => n,
                None => {
                    self.move_to_next_buffer();
                    current_queue = self.load_head_of_queue();
                    current_queue.buffer.get(current_queue.head)?
                }
            };
        }

        match n.get_state() {
            NodeState::Set => {
                let data = n.load();
                current_queue.head += 1;

                self.move_to_next_buffer();
                Some(data)
            }
            NodeState::Empty => {
                // TODO
                // Somehow this introduces a couple of problems where Data is actually lost
                let tmp_head_of_queue = self.load_head_of_queue();
                let tmp_head = tmp_head_of_queue.head;

                let (tmp_head_of_queue, tmp_head) = {
                    let (n_queue, result) = BufferList::scan(tmp_head_of_queue, tmp_head);
                    let n_head = match result {
                        Some(n) => n,
                        None => return None,
                    };
                    (n_queue, n_head)
                };

                let tmp_n = tmp_head_of_queue.buffer.get(tmp_head)?;

                let data = tmp_n.load();
                tmp_n.handled();

                Some(data)

                /*
                let mut head_of_queue = self.load_head_of_queue();
                let (tmp_head_of_queue, tmp_head) = BufferList::rescan(
                    self.head_of_queue as *mut BufferList<T>,
                    tmp_head_of_queue,
                    tmp_head,
                );

                let tmp_n = tmp_head_of_queue.buffer.get(tmp_head).unwrap();

                let data = tmp_n.load();
                tmp_n.handled();

                if tmp_head_of_queue.position_in_queue == head_of_queue.position_in_queue
                    && head_of_queue.head == tmp_head
                {
                    head_of_queue.head += 1;
                    self.move_to_next_buffer();
                }

                Some(data)
                */
            }
            _ => None,
        }
    }
}

unsafe impl<T> Send for Receiver<T> {}

impl<T> Debug for Receiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Receiver ()")
    }
}

/// Creates a new empty Queue
pub fn queue<T>() -> (Receiver<T>, Sender<T>) {
    let initial_buffer = BufferList::boxed(std::ptr::null(), 1);
    let initial_ptr = Box::into_raw(initial_buffer);

    let tail = Arc::new(atomic::AtomicUsize::new(0));
    let tail_of_queue = Arc::new(atomic::AtomicPtr::new(initial_ptr));

    (
        Receiver {
            head_of_queue: initial_ptr as *const BufferList<T>,
        },
        Sender {
            tail,
            tail_of_queue,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dequeue_empty() {
        let (mut rx, _) = queue::<u8>();

        assert_eq!(None, rx.dequeue());
    }

    #[test]
    fn enqueue_one() {
        let (_, tx) = queue();

        tx.enqueue(13);
    }

    #[test]
    fn enqueue_dequeue() {
        let (mut rx, tx) = queue();

        tx.enqueue(13);
        assert_eq!(Some(13), rx.dequeue());
    }

    #[test]
    fn enqueue_fill_one_buffer() {
        let (mut rx, tx) = queue();

        let elements = BUFFER_SIZE + 2;
        for i in 0..elements {
            tx.enqueue(i);
        }
        for i in 0..elements {
            assert_eq!(Some(i), rx.dequeue());
        }
    }

    #[test]
    fn fill_mulitple_buffers() {
        let (mut rx, mut tx) = queue();

        let elements = BUFFER_SIZE * 5;
        for i in 0..elements {
            tx.enqueue(i);
        }
        for i in 0..elements {
            assert_eq!(Some(i), rx.dequeue());
        }

        // make sure it still works after this
        tx.enqueue(13);
        assert_eq!(Some(13), rx.dequeue());
    }
}
