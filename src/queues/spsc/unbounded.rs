//! An unbounded lock-free Queue
//!
//! # Example
//! ```
//! use nolock::queues::spsc::unbounded;
//!
//! // Create a new UnboundedQueue
//! let (mut rx, mut tx) = unbounded::queue();
//!
//! // Enqueue 13
//! tx.enqueue(13);
//! // Dequeue the 13 again
//! assert_eq!(Ok(13), rx.try_dequeue());
//! ```
//!
//! # Reference:
//! * [An Efficient Unbounded Lock-Free Queue - for Multi-core Systems](https://link.springer.com/content/pdf/10.1007%2F978-3-642-32820-6_65.pdf)

mod d_spsc;

use alloc::sync::Arc;
use core::{fmt::Debug, sync::atomic};

use super::bounded;
use crate::queues::{DequeueError, EnqueueError};

#[cfg(feature = "async")]
mod async_queue;
#[cfg(feature = "async")]
pub use async_queue::*;

// TODO
// Add Support for the Caches to improve the Performance and reduce the overhead
// of the Allocator

/// The Sender-Half of an unbounded Queue
pub struct UnboundedSender<T> {
    /// Indicates if the Queue has been closed or not
    closed: Arc<atomic::AtomicBool>,
    /// The Size of each Buffer
    buffer_size: usize,
    /// The current Buffer, where we insert entries
    buf_w: bounded::BoundedSender<T>,
    /// This is used to inform the Consumer aboutu any new Buffers we allocate
    /// in case the current one becomes full
    inuse_sender: d_spsc::UnboundedSender<bounded::BoundedReceiver<T>>,
}

impl<T> UnboundedSender<T> {
    /// Checks if the Queue has been closed by the Consumer
    ///
    /// # Example
    /// ```
    /// # use nolock::queues::spsc::unbounded;
    /// let (rx, tx) = unbounded::queue::<usize>();
    ///
    /// // Drop the Consumer and therefore also close the Queue
    /// drop(rx);
    ///
    /// assert_eq!(true, tx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.closed.load(atomic::Ordering::Acquire)
    }

    /// Creates a new BoundedQueue and sends the Receiving half of the new
    /// BoundedQueue to the Consumer, using the `inuse_sender`.
    fn next_w(&mut self) -> bounded::BoundedSender<T> {
        // Creates the new BoundedQueue with the configured BufferSize
        let (rx, tx) = bounded::queue(self.buffer_size);
        // Sends the Receiving half of the newly created BoundedQueue to the
        // Consumer half
        self.inuse_sender.enqueue(rx).unwrap();
        // Return the sending Half to the caller
        tx
    }

    /// Enqueues the Data
    ///
    /// # Example
    /// Normal Enqueue, where the Queue is not closed
    /// ```
    /// # use nolock::queues::spsc::unbounded;
    /// let (rx, mut tx) = unbounded::queue::<usize>();
    ///
    /// assert_eq!(Ok(()), tx.enqueue(13));
    ///
    /// # drop(rx);
    /// ```
    ///
    /// Failed Enqueue, the Queue has been closed
    /// ```
    /// # use nolock::queues::spsc::unbounded;
    /// # use nolock::queues::EnqueueError;
    /// let (rx, mut tx) = unbounded::queue::<usize>();
    ///
    /// // Drop the Consumer and therefore also close the Queue
    /// drop(rx);
    ///
    /// assert_eq!(Err((13, EnqueueError::Closed)), tx.enqueue(13));
    /// ```
    pub fn enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
        if self.is_closed() {
            return Err((data, EnqueueError::Closed));
        }

        // Attempt to enqueue the Data into the current BoundedQueue.
        //
        // NOTE:
        // We first assume that the current BoundedQueue has still room as this
        // will be the case most of the Time and therefore helps to reduce
        // the time taken for the "fastest hot path" without impacting the
        // alternative very much
        //
        // If this fails, we know that the BoundedQueue has to be full and
        // therefore we start the process to create a new Buffer
        if let Err((data, _)) = self.buf_w.try_enqueue(data) {
            // Create new BoundedQueue and set it as the current BoundedQueue
            // to use for any other writes/enqueues
            self.buf_w = self.next_w();
            // Retry the Enqueue operation with the new BoundedQueue
            //
            // This should always succeed because we just now created the
            // BoundedQueue meaning that is still empty
            if self.buf_w.try_enqueue(data).is_err() {
                panic!("The new Buffer is always empty");
            }
        }

        Ok(())
    }
}

impl<T> Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UnboundedSender ()")
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        self.closed.store(true, atomic::Ordering::Release);
    }
}

unsafe impl<T> Send for UnboundedSender<T> {}
unsafe impl<T> Sync for UnboundedSender<T> {}

/// The Receiver-Half of an unbounded Queue
pub struct UnboundedReceiver<T> {
    /// Indicates if the Queue has been closed or not
    closed: Arc<atomic::AtomicBool>,
    /// The current BoundedQueue from which items are being Dequeued
    buf_r: bounded::BoundedReceiver<T>,
    /// This is used to receive information about any new BoundedQueues created
    /// by the sending Half of this Queue
    inuse_recv: d_spsc::UnboundedReceiver<bounded::BoundedReceiver<T>>,
}

impl<T> UnboundedReceiver<T> {
    /// Checks if the Queue has been closed by the Producer
    ///
    /// # Example
    /// ```
    /// # use nolock::queues::spsc::unbounded;
    /// let (rx, tx) = unbounded::queue::<usize>();
    ///
    /// // Dropping the Producer and therefore closing the Queue
    /// drop(tx);
    ///
    /// assert_eq!(true, rx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.buf_r.is_closed() && !self.inuse_recv.has_next()
    }

    /// Attempts to dequeue a single Element from the Queue
    ///
    /// # Example
    /// Successful Dequeue
    /// ```
    /// # use nolock::queues::spsc::unbounded;
    /// let (mut rx, mut tx) = unbounded::queue::<usize>();
    ///
    /// tx.enqueue(13).unwrap();
    ///
    /// assert_eq!(Ok(13), rx.try_dequeue());
    /// ```
    ///
    /// Dequeue on empty Queue
    /// ```
    /// # use nolock::queues::spsc::unbounded;
    /// # use nolock::queues::DequeueError;
    /// let (mut rx, mut tx) = unbounded::queue::<usize>();
    ///
    /// assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
    ///
    /// # drop(tx);
    /// ```
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Attempt to Dequeue an element from the current BoundedQueue
        match self.buf_r.try_dequeue() {
            // If we dequeued an Item, simply return that and we are done
            Ok(d) => Ok(d),
            // If we receive this Error, we know that the Queue is empty, but
            // the Producer has not moved on to another Queue, as it would then
            // be considered Closed and would return a different Error
            Err(DequeueError::Empty) => Err(DequeueError::Empty),
            // This indicates that the Producer has dropped the current Queue,
            // which indicates that they either moved on to a new Queue already,
            // as this one had been completly filled before, or that the entire
            // Producer has been dropped.
            //
            // We therefore attempt to get the Next queue, to which the
            // Producer would have moved on
            Err(DequeueError::Closed) => match self.inuse_recv.try_dequeue() {
                // If we find a new Queue, we can simply replace our old one
                // with the new one and then attempt to Dequeue th first
                // Element of it as our result
                Ok(n_queue) => {
                    self.buf_r = n_queue;
                    self.buf_r.try_dequeue()
                }
                // If we cant find a new Queue, that means that the Producer
                // has been closed and therefore the entire Queue is now also
                // closed as there are no more Entries left in the Queue
                Err(_) => Err(DequeueError::Closed),
            },
        }
    }

    /// A simple blocking dequeue operation. This is not lock-free anymore
    /// (obviously) and simply spins while trying to dequeue an element from
    /// the Queue until it succeeds
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
}

impl<T> Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "UnboundedReceiver ()")
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        self.closed.store(true, atomic::Ordering::Release);
    }
}

unsafe impl<T> Send for UnboundedReceiver<T> {}
unsafe impl<T> Sync for UnboundedReceiver<T> {}

/// Creates a new Queue
pub fn queue<T>() -> (UnboundedReceiver<T>, UnboundedSender<T>) {
    let buffer_size = 64;

    let (inuse_rx, inuse_tx) = d_spsc::unbounded_basic_queue();
    let (initial_rx, initial_tx) = bounded::queue(buffer_size);

    let closed = Arc::new(atomic::AtomicBool::new(false));

    (
        UnboundedReceiver {
            closed: closed.clone(),
            buf_r: initial_rx,
            inuse_recv: inuse_rx,
        },
        UnboundedSender {
            closed,
            buffer_size,
            buf_w: initial_tx,
            inuse_sender: inuse_tx,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_dequeue() {
        let (mut rx, mut tx) = queue();

        tx.enqueue(13).unwrap();
        assert_eq!(Ok(13), rx.try_dequeue());
    }

    #[test]
    fn multi_buffer() {
        let (mut rx, mut tx) = queue();

        tx.enqueue(13).unwrap();
        tx.enqueue(14).unwrap();
        tx.enqueue(15).unwrap();

        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Ok(14), rx.try_dequeue());
        assert_eq!(Ok(15), rx.try_dequeue());
    }

    #[test]
    fn enqueue_closed() {
        let (rx, mut tx) = queue();
        drop(rx);

        assert_eq!(Err((13, EnqueueError::Closed)), tx.enqueue(13));
    }
    #[test]
    fn dequeue_closed() {
        let (mut rx, tx) = queue::<usize>();
        drop(tx);

        assert_eq!(Err(DequeueError::Closed), rx.try_dequeue());
    }
}
