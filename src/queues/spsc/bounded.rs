//! This implements a bounded lock-free Queue
//!
//! # Reference:
//! * [FastForward for Efficient Pipeline Parallelism - A Cache-Optimized Concurrent Lock-Free Queue](https://www.researchgate.net/publication/213894711_FastForward_for_Efficient_Pipeline_Parallelism_A_Cache-Optimized_Concurrent_Lock-Free_Queue)

use std::{
    cell::UnsafeCell,
    fmt::Debug,
    sync::{atomic, Arc},
};

use super::{DequeueError, EnqueueError};

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
    /// The Index of the next Node to read in the Buffer
    head: usize,
    /// The underlying Buffer of Nodes
    buffer: Arc<Vec<Node<T>>>,
}

/// The Receiving-Half for the Queue
pub struct BoundedReceiver<T> {
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
    /// Attempts to Enqueue the given piece of Data
    pub fn try_enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
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

impl<T> BoundedReceiver<T> {
    /// Attempts to Dequeue the given piece of Data
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Get the Node where would read the next Item from
        let buffer_entry = &self.buffer[self.tail];

        // If the Node is not set, we should return an Error as the Queue is
        // empty and there is nothing for us to return in this Operation
        if !buffer_entry.is_set() {
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

/// Creates a new Bounded-Queue with the given Size
pub fn bounded_queue<T>(size: usize) -> (BoundedReceiver<T>, BoundedSender<T>) {
    // Create the underlying Buffer of Nodes and fill it up with empty Nodes
    // as the initial Configuration
    let mut raw_buffer = Vec::with_capacity(size);
    for _ in 0..size {
        raw_buffer.push(Node::new());
    }

    let buffer = Arc::new(raw_buffer);

    (
        BoundedReceiver {
            buffer: buffer.clone(),
            tail: 0,
        },
        BoundedSender { buffer, head: 0 },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_dequeue() {
        let (mut rx, mut tx) = bounded_queue(10);

        assert_eq!(Ok(()), tx.try_enqueue(13));
        assert_eq!(Ok(13), rx.try_dequeue());
    }
    #[test]
    fn enqueue_will_block() {
        let (_, mut tx) = bounded_queue(1);

        assert_eq!(Ok(()), tx.try_enqueue(13));
        assert_eq!(Err((14, EnqueueError::WouldBlock)), tx.try_enqueue(14));
    }
    #[test]
    fn dequeue_will_block() {
        let (mut rx, _) = bounded_queue::<usize>(1);

        assert_eq!(Err(DequeueError::WouldBlock), rx.try_dequeue());
    }

    #[test]
    fn enqueue_dequeue_full_buffer() {
        let (mut rx, mut tx) = bounded_queue(3);

        for i in 0..4 {
            assert_eq!(Ok(()), tx.try_enqueue(i));
            assert_eq!(Ok(i), rx.try_dequeue());
        }
    }
}
