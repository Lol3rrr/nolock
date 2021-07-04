//! This implements a bounded lock-free Queue
//!
//! # Example
//! ```
//! use nolock::queues::spsc::bounded;
//!
//! // Creates a new BoundedQueue with the Capacity for 5 Items
//! let (mut rx, mut tx) = bounded::queue(5);
//!
//! // Enqueues the Value 13 on the Queue
//! tx.try_enqueue(13);
//! // Dequeues 13 from the Queue again
//! assert_eq!(Ok(13), rx.try_dequeue());
//! ```
//!
//! # Reference:
//! * [FastForward for Efficient Pipeline Parallelism - A Cache-Optimized Concurrent Lock-Free Queue](https://www.researchgate.net/publication/213894711_FastForward_for_Efficient_Pipeline_Parallelism_A_Cache-Optimized_Concurrent_Lock-Free_Queue)

use std::{
    cell::UnsafeCell,
    fmt::Debug,
    sync::{atomic, Arc},
};

use super::{DequeueError, EnqueueError};

#[cfg(feature = "async")]
mod async_queue;
#[cfg(feature = "async")]
pub use async_queue::*;

/// A Node is a single Entry in the Buffer of the Queue
struct Node<T> {
    /// The actual Data stored in the Node itself
    data: UnsafeCell<Option<T>>,
    /// Indicates whether or not the current Node actually contains Data
    is_set: atomic::AtomicBool,
}

impl<T> Node<T> {
    /// Creates a new Empty Node
    pub fn new() -> Self {
        Self {
            data: UnsafeCell::new(None),
            is_set: atomic::AtomicBool::new(false),
        }
    }

    /// Checks if the current Node is marked as `set` and actually contains
    /// Data that could be read
    pub fn is_set(&self) -> bool {
        self.is_set.load(atomic::Ordering::Acquire)
    }

    /// Stores the given Data into the current Node and marks the Node as being
    /// `set` and ready to be consumed
    pub fn store(&self, data: T) {
        // Get the mutable access to the underlying Data in order to overwrite
        // it with the given new Data
        let d_ptr = self.data.get();
        let mut_data = unsafe { &mut *d_ptr };

        // Actually store the Data into the Node
        mut_data.replace(data);

        // Mark the Node as `set` again
        self.is_set.store(true, atomic::Ordering::Release);
    }

    /// Attempts to load the current Data from the Node and marks the Data as
    /// empty again
    pub fn load(&self) -> T {
        // Get the mutable access to the underlying Data in order to properly
        // take it out and replace it with empty Data
        let d_ptr = self.data.get();
        let mut_data = unsafe { &mut *d_ptr };

        // Take the Data out of the Option
        let data = mut_data.take().unwrap();
        // Mark the Node as empty again
        self.is_set.store(false, atomic::Ordering::Release);

        // Return the Data
        data
    }
}

/// The Sending-Half for the queue
pub struct BoundedSender<T> {
    /// Indicates if the Queue has been closed or not
    closed: Arc<atomic::AtomicBool>,
    /// The Index of the next Node to read in the Buffer
    head: usize,
    /// The underlying Buffer of Nodes
    buffer: Arc<Vec<Node<T>>>,
}

/// The Receiving-Half for the Queue
pub struct BoundedReceiver<T> {
    /// Indicates if the Queue has been closed or not
    closed: Arc<atomic::AtomicBool>,
    /// The Index of the next Node to store Data into
    tail: usize,
    /// The underlying Buffer of Nodes
    buffer: Arc<Vec<Node<T>>>,
}

/// Calculates the Index of the next Element in the Buffer and wraps around
/// if the End of the Buffer has been reached
#[inline(always)]
const fn next_element(current: usize, length: usize) -> usize {
    // This code is logically speaking identical to simply returning
    // `(current + 1) % length`
    // However after performing some benchmarks, this code seems to be
    // significantly, in this context, faster than the simple naive variant
    let target = current + 1;
    if target >= length {
        0
    } else {
        target
    }
}

impl<T> BoundedSender<T> {
    /// Returns whether or not the Queue has been closed by the other Side
    pub fn is_closed(&self) -> bool {
        self.closed.load(atomic::Ordering::Acquire)
    }

    /// Attempts to Enqueue the given piece of Data
    pub fn try_enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
        if self.is_closed() {
            return Err((data, EnqueueError::Closed));
        }

        // Get a reference to the current Entry where we would enqueue the next
        // Element
        let buffer_entry = &self.buffer[self.head];

        // If the Node is already set, that means we don't have anywhere to
        // store the new Element, meaning that the Buffer is full and we should
        // Error out indicating this
        if buffer_entry.is_set() {
            return Err((data, EnqueueError::WouldBlock));
        }

        // The Node is not already set meaning that we can simply store the
        // given Data into the Node
        buffer_entry.store(data);

        // Advance the current Head, where we insert the Elements, onto the
        // next Position
        self.head = next_element(self.head, self.buffer.len());

        // Return Ok to indicate a successful enqueue operation
        Ok(())
    }

    /// A blocking enqueue Operation. This is obviously not lock-free anymore
    /// and will simply spin while trying to enqueue the Data until it works
    pub fn enqueue(&mut self, mut data: T) -> Result<(), (T, EnqueueError)> {
        loop {
            match self.try_enqueue(data) {
                Ok(_) => return Ok(()),
                Err((d, e)) => match e {
                    EnqueueError::WouldBlock => {
                        data = d;
                    }
                    EnqueueError::Closed => return Err((d, EnqueueError::Closed)),
                },
            };
        }
    }

    /// Checks if the current Queue is full
    pub fn is_full(&self) -> bool {
        // If the Node where we would insert the next Element is already set
        // that means, that there is currently no room for new Elements in the
        // Queue, meaning that the Queue is currently full
        self.buffer[self.head].is_set()
    }
}

impl<T> Debug for BoundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BoundedSender ()")
    }
}

impl<T> Drop for BoundedSender<T> {
    fn drop(&mut self) {
        self.closed.store(true, atomic::Ordering::Release);
    }
}

unsafe impl<T> Send for BoundedSender<T> {}
unsafe impl<T> Sync for BoundedSender<T> {}

impl<T> BoundedReceiver<T> {
    /// Checks if the Queue has been  closed by the other Side
    pub fn is_closed(&self) -> bool {
        self.closed.load(atomic::Ordering::Acquire)
    }

    /// Attempts to Dequeue the given piece of Data
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Get the Node where would read the next Item from
        let buffer_entry = &self.buffer[self.tail];

        // If the Node is not set, we should return an Error as the Queue is
        // empty and there is nothing for us to return in this Operation
        if !buffer_entry.is_set() {
            if self.is_closed() {
                return Err(DequeueError::Closed);
            }

            return Err(DequeueError::WouldBlock);
        }

        // If the Node is set, we can load the Data out of the Node itself
        let data = buffer_entry.load();

        // Advance the current Tail, indicating where we should read the next
        // Element from, onto the next Node in the Buffer
        self.tail = next_element(self.tail, self.buffer.len());

        // Return `Ok` with the Data
        Ok(data)
    }

    /// A blocking dequeue operations. This is not lock-free anymore and simply
    /// spins while trying to dequeue until it works.
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

    /// Checks if the current queue is empty
    pub fn is_empty(&self) -> bool {
        // If the current Node where would dequeue the next Item from is not
        // marked as being set, the Node contains no `set` Nodes and therefore
        // the Queue is currently empty
        !self.buffer[self.tail].is_set()
    }
}

impl<T> Debug for BoundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BoundedReceiver ()")
    }
}

impl<T> Drop for BoundedReceiver<T> {
    fn drop(&mut self) {
        self.closed.store(true, atomic::Ordering::Release);
    }
}

unsafe impl<T> Send for BoundedReceiver<T> {}
unsafe impl<T> Sync for BoundedReceiver<T> {}

/// Creates a new Bounded-Queue with the given Size
pub fn queue<T>(size: usize) -> (BoundedReceiver<T>, BoundedSender<T>) {
    // Create the underlying Buffer of Nodes and fill it up with empty Nodes
    // as the initial Configuration
    let mut raw_buffer = Vec::with_capacity(size);
    for _ in 0..size {
        raw_buffer.push(Node::new());
    }

    let closed = Arc::new(atomic::AtomicBool::new(false));
    let buffer = Arc::new(raw_buffer);

    (
        BoundedReceiver {
            closed: closed.clone(),
            buffer: buffer.clone(),
            tail: 0,
        },
        BoundedSender {
            closed,
            buffer,
            head: 0,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_dequeue() {
        let (mut rx, mut tx) = queue(10);

        assert_eq!(Ok(()), tx.try_enqueue(13));
        assert_eq!(Ok(13), rx.try_dequeue());
    }
    #[test]
    fn enqueue_will_block() {
        let (rx, mut tx) = queue(1);

        assert_eq!(Ok(()), tx.try_enqueue(13));
        assert_eq!(Err((14, EnqueueError::WouldBlock)), tx.try_enqueue(14));

        drop(rx);
    }
    #[test]
    fn dequeue_will_block() {
        let (mut rx, tx) = queue::<usize>(1);

        assert_eq!(Err(DequeueError::WouldBlock), rx.try_dequeue());

        drop(tx);
    }

    #[test]
    fn enqueue_dequeue_full_buffer() {
        let (mut rx, mut tx) = queue(3);

        for i in 0..4 {
            assert_eq!(Ok(()), tx.try_enqueue(i));
            assert_eq!(Ok(i), rx.try_dequeue());
        }
    }

    #[test]
    fn enqueue_is_closed() {
        let (rx, mut tx) = queue(3);

        drop(rx);
        assert_eq!(Err((13, EnqueueError::Closed)), tx.try_enqueue(13));
    }
    #[test]
    fn dequeue_is_closed() {
        let (mut rx, tx) = queue::<usize>(3);

        drop(tx);
        assert_eq!(Err(DequeueError::Closed), rx.try_dequeue());
    }
    #[test]
    fn enqueue_dequeue_is_closed() {
        let (mut rx, mut tx) = queue::<usize>(3);

        tx.try_enqueue(13).unwrap();
        drop(tx);

        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Err(DequeueError::Closed), rx.try_dequeue());
    }

    #[test]
    fn blocking_enqueue_closed() {
        let (rx, mut tx) = queue::<usize>(3);
        drop(rx);

        assert_eq!(Err((13, EnqueueError::Closed)), tx.enqueue(13));
    }
    #[test]
    fn blocking_dequeue_closed() {
        let (mut rx, tx) = queue::<usize>(3);
        drop(tx);

        assert_eq!(None, rx.dequeue());
    }
}
