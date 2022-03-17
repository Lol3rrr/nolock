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

use alloc::{sync::Arc, vec::Vec};
use core::{fmt::Debug, sync::atomic};

use crate::queues::{DequeueError, EnqueueError};

#[cfg(feature = "async")]
mod async_queue;
#[cfg(feature = "async")]
pub use async_queue::*;

mod node;
use node::Node;

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
    /// Returns whether or not the Queue has been closed by the Consumer
    ///
    /// # Example
    /// ```
    /// # use nolock::queues::spsc::bounded;
    /// let (rx, tx) = bounded::queue::<usize>(3);
    ///
    /// // Drop the Consumer and therefore also close the Queue
    /// drop(rx);
    ///
    /// assert_eq!(true, tx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.closed.load(atomic::Ordering::Acquire)
    }

    /// Attempts to Enqueue the given piece of Data
    ///
    /// # Example:
    /// Enqueue Data when there is still space
    /// ```
    /// # use nolock::queues::spsc::bounded;
    /// // Create a new Queue with the capacity for 16 Elements
    /// let (mut rx, mut tx) = bounded::queue::<usize>(16);
    ///
    /// // Enqueue some Data
    /// assert_eq!(Ok(()), tx.try_enqueue(13));
    ///
    /// # assert_eq!(Ok(13), rx.try_dequeue());
    /// ```
    ///
    /// Enqueue Data when there is no more space
    /// ```
    /// # use nolock::queues::spsc::bounded;
    /// # use nolock::queues::EnqueueError;
    /// // Create a new Queue with the capacity for 16 Elements
    /// let (mut rx, mut tx) = bounded::queue::<usize>(16);
    ///
    /// // Fill up the Queue
    /// for i in 0..16 {
    ///   assert_eq!(Ok(()), tx.try_enqueue(i));
    /// }
    ///
    /// // Attempt to enqueue some Data, but there is no more room
    /// assert_eq!(Err((13, EnqueueError::Full)), tx.try_enqueue(13));
    ///
    /// # drop(rx);
    /// ```
    pub fn try_enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
        if self.is_closed() {
            return Err((data, EnqueueError::Closed));
        }

        // Get a reference to the current Entry where we would enqueue the next
        // Element
        let buffer_entry = unsafe { self.buffer.get_unchecked(self.head) };

        // If the Node is already set, that means we don't have anywhere to
        // store the new Element, meaning that the Buffer is full and we should
        // Error out indicating this
        if buffer_entry.is_set() {
            return Err((data, EnqueueError::Full));
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
                    EnqueueError::Full => {
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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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
    /// Checks if the Queue has been closed by the Producer
    ///
    /// # Note
    /// Even when this indicates that the Queue has been closed, there might
    /// still be Items in the Queue left that should first be dequeued by the
    /// Consumer before discarding the entire Queue
    ///
    /// # Example
    /// ```
    /// # use nolock::queues::spsc::bounded;
    /// let (rx, tx) = bounded::queue::<usize>(3);
    ///
    /// // Drop the Producer and therefore also close the Queue
    /// drop(tx);
    ///
    /// assert_eq!(true, rx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.closed.load(atomic::Ordering::Acquire)
    }

    /// Attempts to Dequeue a single Element from the Queue
    ///
    /// # Example
    /// There was something to dequeu
    /// ```
    /// # use nolock::queues::spsc::bounded;
    /// // Create a new Queue with the Capacity for 16-Elements
    /// let (mut rx, mut tx) = bounded::queue::<usize>(16);
    ///
    /// // Enqueue the Element
    /// tx.try_enqueue(13);
    ///
    /// // Dequeue the Element again
    /// assert_eq!(Ok(13), rx.try_dequeue());
    /// ```
    ///
    /// The Queue is empty and therefore nothing could be dequeued
    /// ```
    /// # use nolock::queues::spsc::bounded;
    /// # use nolock::queues::DequeueError;
    /// // Create a new Queue with the Capacity for 16-Elements
    /// let (mut rx, mut tx) = bounded::queue::<usize>(16);
    ///
    /// // Dequeue the Element again
    /// assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
    ///
    /// # drop(tx);
    /// ```
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Get the Node where would read the next Item from
        let buffer_entry = unsafe { self.buffer.get_unchecked(self.tail) };

        // If the Node is not set, we should return an Error as the Queue is
        // empty and there is nothing for us to return in this Operation
        if !buffer_entry.is_set() {
            // Check if the Queue has been marked as closed
            if self.is_closed() {
                // We need to recheck the current Node, because it may have
                // been set in the mean time and then the closed flag was
                // updated
                if !buffer_entry.is_set() {
                    return Err(DequeueError::Closed);
                }
            }

            return Err(DequeueError::Empty);
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
                    DequeueError::Empty => {}
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
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

/// Creates a new Bounded-Queue with the given Capacity and returns the
/// corresponding Handles ([`BoundedReceiver`], [`BoundedSender`])
pub fn queue<T>(capacity: usize) -> (BoundedReceiver<T>, BoundedSender<T>) {
    // Create the underlying Buffer of Nodes and fill it up with empty Nodes
    // as the initial Configuration
    let mut raw_buffer = Vec::with_capacity(capacity);
    for _ in 0..capacity {
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
        assert_eq!(Err((14, EnqueueError::Full)), tx.try_enqueue(14));

        drop(rx);
    }
    #[test]
    fn dequeue_will_block() {
        let (mut rx, tx) = queue::<usize>(1);

        assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());

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

    #[test]
    fn is_empty() {
        let (mut rx, mut tx) = queue::<usize>(3);

        assert_eq!(true, rx.is_empty());

        tx.try_enqueue(13).unwrap();
        assert_eq!(false, rx.is_empty());

        rx.try_dequeue().unwrap();
        assert_eq!(true, rx.is_empty());
    }

    #[test]
    fn is_full() {
        let (mut rx, mut tx) = queue::<usize>(1);

        assert_eq!(false, tx.is_full());

        tx.try_enqueue(13).unwrap();
        assert_eq!(true, tx.is_full());

        rx.try_dequeue().unwrap();
        assert_eq!(false, tx.is_full());
    }
}
