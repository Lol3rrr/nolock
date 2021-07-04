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

use std::fmt::Debug;

use super::{bounded, DequeueError};

// TODO
// Add Support for the Caches to improve the Performance and reduce the overhead
// of the Allocator

/// The Sender-Half of an unbounded Queue
pub struct UnboundedSender<T> {
    /// The Size of each Buffer
    buffer_size: usize,
    /// The current Buffer, where we insert entries
    buf_w: bounded::BoundedSender<T>,
    /// This is used to inform the Consumer aboutu any new Buffers we allocate
    /// in case the current one becomes full
    inuse_sender: d_spsc::UnboundedSender<bounded::BoundedReceiver<T>>,
}

impl<T> UnboundedSender<T> {
    /// Creates a new BoundedQueue and sends the Receiving half of the new
    /// BoundedQueue to the Consumer, using the `inuse_sender`.
    fn next_w(&mut self) -> bounded::BoundedSender<T> {
        // Creates the new BoundedQueue with the configured BufferSize
        let (rx, tx) = bounded::queue(self.buffer_size);
        // Sends the Receiving half of the newly created BoundedQueue to the
        // Consumer half
        self.inuse_sender.enqueue(rx);
        // Return the sending Half to the caller
        tx
    }

    /// Enqueues the Data
    pub fn enqueue(&mut self, data: T) {
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
    }
}

impl<T> Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedSender ()")
    }
}

unsafe impl<T> Send for UnboundedSender<T> {}
unsafe impl<T> Sync for UnboundedSender<T> {}

/// The Receiver-Half of an unbounded Queue
pub struct UnboundedReceiver<T> {
    /// The current BoundedQueue from which items are being Dequeued
    buf_r: bounded::BoundedReceiver<T>,
    /// This is used to receive information about any new BoundedQueues created
    /// by the sending Half of this Queue
    inuse_recv: d_spsc::UnboundedReceiver<bounded::BoundedReceiver<T>>,
}

impl<T> UnboundedReceiver<T> {
    /// Attempts to dequeue a single Element from the Queue
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Attempt to Dequeue an element from the current BoundedQueue
        match self.buf_r.try_dequeue() {
            // If we dequeued an Item, simply return that and we are done
            Ok(d) => Ok(d),
            // If we failed to dequeue an item, there are two possible
            // situations:
            // * The currently used BoundedQueue is empty and the producer has
            //   moved on to a new BoundedQueue for all further elements
            // * The entire Queue is simply empty
            //
            // To resolve this we attempt to dequeue a new BoundedQueue, which
            // should only work if the Producer has moved on
            Err(_) => match self.inuse_recv.try_dequeue() {
                // The Producer moved on from the previous Buffer and will
                // continue inserting Items into this new BoundedQueue until
                // this one is eventually full as well
                Ok(n_queue) => {
                    // Replace the currently held BoundedQueue with the newly
                    // received One
                    self.buf_r = n_queue;
                    // Attempt to dequeue from this new BoundedQueue and then
                    self.buf_r.try_dequeue()
                }
                // There is no other Queue that the Producer moved on to,
                // meaning that the entire Queue is simply empty so we should
                // return the right Error and exit
                Err(_) => Err(DequeueError::WouldBlock),
            },
        }
    }

    /// A simple blocking dequeue operation. This is not lock-free anymore
    /// (obviously) and simply spins while trying to dequeue an element from
    /// the Queue until it succeeds
    pub fn dequeue(&mut self) -> Option<T> {
        loop {
            if let Ok(data) = self.try_dequeue() {
                return Some(data);
            }
        }
    }
}

impl<T> Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedReceiver ()")
    }
}

unsafe impl<T> Send for UnboundedReceiver<T> {}
unsafe impl<T> Sync for UnboundedReceiver<T> {}

/// Creates a new Queue
pub fn queue<T>() -> (UnboundedReceiver<T>, UnboundedSender<T>) {
    let buffer_size = 64;

    let (inuse_rx, inuse_tx) = d_spsc::unbounded_basic_queue();
    let (initial_rx, initial_tx) = bounded::queue(buffer_size);

    (
        UnboundedReceiver {
            buf_r: initial_rx,
            inuse_recv: inuse_rx,
        },
        UnboundedSender {
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

        tx.enqueue(13);
        assert_eq!(Ok(13), rx.try_dequeue());
    }

    #[test]
    fn multi_buffer() {
        let (mut rx, mut tx) = queue();

        tx.enqueue(13);
        tx.enqueue(14);
        tx.enqueue(15);

        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Ok(14), rx.try_dequeue());
        assert_eq!(Ok(15), rx.try_dequeue());
    }
}
