//! An unbounded lock-free Queue
//!
//! # Reference:
//! * [An Efficient Unbounded Lock-Free Queue - for Multi-core Systems](https://link.springer.com/content/pdf/10.1007%2F978-3-642-32820-6_65.pdf)

pub mod d_spsc;

use std::fmt::Debug;

use super::{bounded, DequeueError};

// TODO
// Add Support for the Caches to improve the Performance and reduce the overhead
// of the Allocator

/// The Sender-Half of an unbounded Queue
pub struct UnboundedSender<T> {
    buffer_size: usize,
    buf_w: bounded::BoundedSender<T>,
    inuse_sender: d_spsc::UnboundedSender<bounded::BoundedReceiver<T>>,
}

impl<T> UnboundedSender<T> {
    fn next_w(&mut self) -> bounded::BoundedSender<T> {
        let (rx, tx) = bounded::bounded_queue(self.buffer_size);
        self.inuse_sender.enqueue(rx);
        tx
    }

    /// Enqueues the Data
    pub fn enqueue(&mut self, data: T) {
        if self.buf_w.is_full() {
            self.buf_w = self.next_w();
        }
        if self.buf_w.try_enqueue(data).is_err() {
            panic!("The new Buffer should always have capacity for a new Element");
        }
    }
}

impl<T> Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedSender ()")
    }
}

/// The Receiver-Half of an unbounded Queue
pub struct UnboundedReceiver<T> {
    buf_r: bounded::BoundedReceiver<T>,
    inuse_recv: d_spsc::UnboundedReceiver<bounded::BoundedReceiver<T>>,
}

impl<T> UnboundedReceiver<T> {
    fn next_r(&mut self) -> Option<bounded::BoundedReceiver<T>> {
        match self.inuse_recv.try_dequeue() {
            Ok(b) => Some(b),
            Err(_) => None,
        }
    }

    /// Attempts to dequeue a single Element from the Queue
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        if self.buf_r.is_empty() {
            if self.inuse_recv.is_empty() {
                return Err(DequeueError::WouldBlock);
            }
            if self.buf_r.is_empty() {
                self.buf_r = match self.next_r() {
                    Some(b) => b,
                    None => return Err(DequeueError::WouldBlock),
                };
            }
        }

        self.buf_r.try_dequeue()
    }
}

impl<T> Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedReceiver ()")
    }
}

/// Creates a new Queue
pub fn unbounded_queue<T>(buffer_size: usize) -> (UnboundedReceiver<T>, UnboundedSender<T>) {
    let (inuse_rx, inuse_tx) = d_spsc::unbounded_basic_queue();
    let (initial_rx, initial_tx) = bounded::bounded_queue(buffer_size);

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
        let (mut rx, mut tx) = unbounded_queue(10);

        tx.enqueue(13);
        assert_eq!(Ok(13), rx.try_dequeue());
    }

    #[test]
    fn multi_buffer() {
        let (mut rx, mut tx) = unbounded_queue(1);

        tx.enqueue(13);
        tx.enqueue(14);
        tx.enqueue(15);

        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Ok(14), rx.try_dequeue());
        assert_eq!(Ok(15), rx.try_dequeue());
    }
}
