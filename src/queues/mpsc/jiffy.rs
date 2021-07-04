//! The implemenation of a Lock-Free, possibly Wait-Free, unbounded MPSC Queue
//!
//! # Examples:
//! ```rust
//! use nolock::queues::mpsc::jiffy;
//!
//! // Create a new Queue
//! let (mut rx, tx) = jiffy::queue();
//!
//! // Enqueue some Data
//! tx.enqueue(13);
//!
//! // Dequeue the Data again
//! assert_eq!(Ok(13), rx.try_dequeue());
//! ```
//!
//! # Reference:
//! * [Jiffy: A Fast, Memory Efficient, Wait-Free Multi-Producers Single-Consumer Queue](https://arxiv.org/pdf/2010.14189.pdf)

use std::{
    fmt::Debug,
    mem::ManuallyDrop,
    sync::{atomic, Arc},
};

/// The Size of each Buffer in the "BufferList"
const BUFFER_SIZE: usize = 256;

mod node;
use node::NodeState;

mod bufferlist;
use bufferlist::BufferList;

#[cfg(feature = "async")]
mod async_queue;
#[cfg(feature = "async")]
pub use async_queue::*;

use super::{DequeueError, EnqueueError};

/// One of the Sender, created by calling [`queue`]
pub struct Sender<T> {
    /// Indicates if the Queue has been closed
    closed: Arc<atomic::AtomicBool>,
    /// This is a shared Usize that Points to the Location in the overall
    /// Buffer-List, where the next Item should be enqueued
    tail: atomic::AtomicUsize,
    /// This is a shared Pointer to the Last Buffer in the Buffer-List
    tail_of_queue: atomic::AtomicPtr<BufferList<T>>,
}

/// The Single Receiver of a Jiffy-Queue, created by calling [`queue`]
pub struct Receiver<T> {
    /// Indicates if the Queue has been closed
    closed: Arc<atomic::AtomicBool>,
    /// This is a simply Ptr to the current Buffer from where items will be
    /// dequeued
    head_of_queue: *const BufferList<T>,
}

impl<T> Sender<T> {
    /// Checks if the Queue has been closed by either Side
    pub fn is_closed(&self) -> bool {
        self.closed.load(atomic::Ordering::Acquire)
    }

    /// Enqueues the given piece of Data
    pub fn enqueue(&self, data: T) -> Result<(), (T, EnqueueError)> {
        if self.is_closed() {
            return Err((data, EnqueueError::Closed));
        }

        // Load our target absolute position, on where to insert the next
        // Element
        //
        // This needs to use Ordering::SeqCst because we would otherwise have
        // one half of the load-store operation be Ordering::Relaxed, which
        // is not what we need
        let location = self.tail.fetch_add(1, atomic::Ordering::SeqCst);

        // Get the current tail-buffer, where we would initially attempt to
        // insert the Element into
        let mut tmp_buffer_ptr = self.tail_of_queue.load(atomic::Ordering::Acquire);
        let mut tmp_buffer = ManuallyDrop::new(unsafe { Box::from_raw(tmp_buffer_ptr) });

        // Get the current End position of the received buffer
        let mut end = tmp_buffer.position_in_queue * BUFFER_SIZE;
        // If the Target-Location is beyond the current Buffer, we need
        // to either create a new Buffer and append it to the Queue or
        // simply walk the List of Buffers in the Queue until we find one
        // that is larger than our Target-Location.
        // However this does not garantuee, that the resulting buffer
        // actually contains our Target-Location, because the buffer we
        // find could come after the Buffer that we actually need
        while location >= end {
            // If the currently loaded Buffer has no next Ptr, meaning
            // it is currently the last Buffer in the Queue, we need to
            // create a new Buffer and append it
            if tmp_buffer.next.load(atomic::Ordering::Acquire).is_null() {
                // Attempt to allocate a new Buffer
                tmp_buffer.allocate_next(tmp_buffer_ptr, &self.tail_of_queue);
            }

            // Load the new Tail of the Queue
            tmp_buffer_ptr = self.tail_of_queue.load(atomic::Ordering::Acquire);
            tmp_buffer = ManuallyDrop::new(unsafe { Box::from_raw(tmp_buffer_ptr) });

            // Recalculate the current End of the new Tail-Buffer
            end = tmp_buffer.position_in_queue * BUFFER_SIZE;
        }

        // Calculate the Starting-Location of the currently loaded
        // Buffer
        let mut start = (tmp_buffer.position_in_queue - 1) * BUFFER_SIZE;
        // If the Target-Location is before the current Buffer's start,
        // we need to move back in the List of Buffers until we find the one
        // that actually contains our Target-Location
        while location < start {
            // Load the previous Buffer in regards to our current one
            tmp_buffer_ptr = tmp_buffer.previous as *mut BufferList<T>;
            tmp_buffer = ManuallyDrop::new(unsafe { Box::from_raw(tmp_buffer_ptr) });

            // Recalculate the Buffers Starting position for the new one
            start = (tmp_buffer.position_in_queue - 1) * BUFFER_SIZE;
        }

        // Calculate the concrete Target-Index in the final Buffer
        let index = location - start;

        // Actually store the Data into the Buffer at the previously
        // calculated Index
        tmp_buffer.buffer[index].store(data);

        // TODO
        // Possible optimization regarding to pre-allocate the next buffer early
        Ok(())
    }
}

impl<T> Debug for Sender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Sender ()")
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.closed.store(true, atomic::Ordering::Release);
    }
}

impl<T> Receiver<T> {
    /// Checks if the Queue has been closed by either Side
    pub fn is_closed(&self) -> bool {
        self.closed.load(atomic::Ordering::Acquire)
    }

    /// Loads the current Head of the Buffer-List
    fn load_head_of_queue(&self) -> ManuallyDrop<Box<BufferList<T>>> {
        let ptr = self.head_of_queue;
        ManuallyDrop::new(unsafe { Box::from_raw(ptr as *mut BufferList<T>) })
    }

    /// Checks if the end of the current Buffer has been reached and if that
    /// is the case, we need to attempt to switch over to the next Buffer in
    /// the List of Buffers
    fn move_to_next_buffer(&mut self) -> bool {
        // Load the current Buffer
        let current_queue = self.load_head_of_queue();

        // If the current Queue has reached its end, we should attempt to
        // switch over to the next Buffer
        if current_queue.head >= BUFFER_SIZE {
            // Lines 63 - 65
            // can be ommited in this case as the next_ptr will then also be 0 and therefore
            // the next check should catch that

            // Load the ptr to the next Buffer from the current Buffer
            let next_ptr = current_queue.next.load(atomic::Ordering::Acquire);
            // If the PTR is null, that means there is currently no next Buffer
            // and we should just return early
            if next_ptr.is_null() {
                return false;
            }

            // Load the current Buffer
            let previous = unsafe { Box::from_raw(self.head_of_queue as *mut BufferList<T>) };

            // Store the next Buffer as the current Buffer
            self.head_of_queue = next_ptr;

            // Drop and therefore free the previously current Buffer
            drop(previous);
        }

        true
    }

    /// Attempts to dequeue the next entry in the Queue
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Loads the current Buffer that should be used
        let mut current_queue = self.load_head_of_queue();

        // Attempt to get the current Entry that we want to dequeue
        let mut n = match current_queue.buffer.get(current_queue.head) {
            Some(n) => n,
            None => {
                // This path is hit, once we reached the end of the current
                // Buffer in the previous dequeue operation but we did not
                // have a next Buffer to load, meaning that we now try to load
                // out of Bounds, meaning that we hit the None case when
                // loading

                // Attempt to move to the next Buffer again
                self.move_to_next_buffer();
                // Reload the current Buffer
                current_queue = self.load_head_of_queue();

                // Retry the loading of the Node, we use the `?` in this case,
                // because if we dont find it again, there is nothing else we
                // can really do and should simply return None as there was
                // currently nothing to load
                match current_queue.buffer.get(current_queue.head) {
                    Some(n) => n,
                    None => return Err(DequeueError::WouldBlock),
                }
            }
        };

        // Find the first node that is not set to Handled
        while n.get_state() == NodeState::Handled {
            current_queue.head += 1;

            if !self.move_to_next_buffer() {
                return Err(DequeueError::WouldBlock);
            }

            current_queue = self.load_head_of_queue();
            n = match current_queue.buffer.get(current_queue.head) {
                Some(n) => n,
                None => {
                    self.move_to_next_buffer();
                    current_queue = self.load_head_of_queue();
                    match current_queue.buffer.get(current_queue.head) {
                        Some(t) => t,
                        None => return Err(DequeueError::WouldBlock),
                    }
                }
            };
        }

        // Load the State of the current Node
        match n.get_state() {
            // If it is Set that means that the Node has Data set and we can
            // simply load the Data from it
            NodeState::Set => {
                // Load the Data from the current Node
                let data = n.load();
                // Advance the Head of the current Buffer to the next Node
                current_queue.head += 1;

                // Move to the next Buffer if we need to
                self.move_to_next_buffer();
                // Return the loaded Data
                Ok(data)
            }
            // If the found Node is set to empty, we should search the rest
            // of the Buffers of the Queue to find if any other Node has been
            // Set and if we find one return that
            NodeState::Empty => {
                // Load the current Head of the Queue
                let tmp_head_of_queue = self.load_head_of_queue();
                let tmp_head = tmp_head_of_queue.head;

                // Look for the next Set Node
                // This returns the Buffer and the Index in the Buffer
                let (tmp_head_of_queue, tmp_head) = {
                    let (mut n_queue, result) = BufferList::scan(tmp_head_of_queue, tmp_head);
                    let n_head = match result {
                        Some(n) => n,
                        // We could not find a Set Node in this pass
                        None => {
                            // Check if the Queue has been marked as closed
                            if self.is_closed() {
                                // If the Queue has been closed, then there are
                                // no more Insertions happending and all
                                // previous ones should have completed.
                                //
                                // We then once again search for a Set-Node to
                                // make sure we don't forget to dequeue any
                                // Node
                                let tmp_head_of_queue = self.load_head_of_queue();
                                let (t_queue, t_result) =
                                    BufferList::scan(tmp_head_of_queue, tmp_head);
                                match t_result {
                                    // We still Found a Set-Node, so we will
                                    // simply continue as if the Queue has
                                    // not been closed yet
                                    Some(n) => {
                                        n_queue = t_queue;
                                        n
                                    }
                                    // We could not find any outstanding Nodes
                                    // in the Buffer and therefore conclude
                                    // that the Buffer is empty and we can
                                    // savely claim that the Buffer has been
                                    // closed and can be discarded
                                    None => return Err(DequeueError::Closed),
                                }
                            } else {
                                return Err(DequeueError::WouldBlock);
                            }
                        }
                    };
                    (n_queue, n_head)
                };

                // Try to load the found Node
                let tmp_n = match tmp_head_of_queue.buffer.get(tmp_head) {
                    Some(n) => n,
                    None => return Err(DequeueError::WouldBlock),
                };

                // Actually load the Data from the Node
                let data = tmp_n.load();
                // Set the Node to being Handled to not accidentally load the
                // same Node twice
                tmp_n.handled();

                Ok(data)

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
            _ => Err(DequeueError::WouldBlock),
        }
    }

    /// This is a simple blocking dequeue. This is definetly not lock free
    /// anymore and will simply spin and try to dequeue an item over and over
    /// again.
    pub fn dequeue(&mut self) -> Option<T> {
        loop {
            match self.try_dequeue() {
                Ok(d) => return Some(d),
                Err(e) => match e {
                    DequeueError::WouldBlock => {}
                    DequeueError::Closed => return None,
                },
            };
        }
    }
}

// These are both save to manually implement because we would garantuee that
// they are save to share across threads, because the algorithm garantuees it
unsafe impl<T> Send for Receiver<T> {}
unsafe impl<T> Sync for Receiver<T> {}

impl<T> Debug for Receiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Receiver ()")
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.closed.store(true, atomic::Ordering::Release);
    }
}

/// Creates a new empty Queue and returns their ([`Receiver`], [`Sender`])
pub fn queue<T>() -> (Receiver<T>, Sender<T>) {
    let initial_buffer = BufferList::boxed(std::ptr::null(), 1);
    let initial_ptr = Box::into_raw(initial_buffer);

    let tail = atomic::AtomicUsize::new(0);
    let tail_of_queue = atomic::AtomicPtr::new(initial_ptr);

    let closed = Arc::new(atomic::AtomicBool::new(false));

    (
        Receiver {
            closed: closed.clone(),
            head_of_queue: initial_ptr as *const BufferList<T>,
        },
        Sender {
            closed,
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
        let (mut rx, tx) = queue::<u8>();

        assert_eq!(Err(DequeueError::WouldBlock), rx.try_dequeue());
        drop(tx);
    }

    #[test]
    fn enqueue_one() {
        let (rx, tx) = queue();

        tx.enqueue(13).unwrap();
        drop(rx);
    }

    #[test]
    fn enqueue_dequeue() {
        let (mut rx, tx) = queue();

        tx.enqueue(13).unwrap();
        assert_eq!(Ok(13), rx.try_dequeue());
    }

    #[test]
    fn enqueue_fill_one_buffer() {
        let (mut rx, tx) = queue();

        let elements = BUFFER_SIZE + 2;
        for i in 0..elements {
            tx.enqueue(i).unwrap();
        }
        for i in 0..elements {
            assert_eq!(Ok(i), rx.try_dequeue());
        }
    }

    #[test]
    fn fill_mulitple_buffers() {
        let (mut rx, tx) = queue();

        let elements = BUFFER_SIZE * 5;
        for i in 0..elements {
            tx.enqueue(i).unwrap();
        }
        for i in 0..elements {
            assert_eq!(Ok(i), rx.try_dequeue());
        }

        // make sure it still works after this
        tx.enqueue(13).unwrap();
        assert_eq!(Ok(13), rx.try_dequeue());
    }

    #[test]
    fn enqueue_closed() {
        let (rx, tx) = queue();
        drop(rx);

        assert_eq!(Err((13, EnqueueError::Closed)), tx.enqueue(13));
    }

    #[test]
    fn dequeue_closed() {
        let (mut rx, tx) = queue::<usize>();
        drop(tx);

        assert_eq!(Err(DequeueError::Closed), rx.try_dequeue());
    }
    #[test]
    fn enqueue_dequeue_closed() {
        let (mut rx, tx) = queue::<usize>();

        tx.enqueue(13).unwrap();
        drop(tx);

        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Err(DequeueError::Closed), rx.try_dequeue());
    }
}
