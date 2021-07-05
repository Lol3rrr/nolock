//! This provides Multi-Producer Single-Consumer queues
//!
//! # Queues
//! ## Jiffy
//! Jiffy is based on a relatively recent Paper, by the same name, which can be
//! found under the "References"-Section in Jiffy's
//! [`module-level documentation`](jiffy).
//! Jiffy is also an Unbounded-Queue, which makes it useful for a wide variety
//! of use-cases, and its good performance characteristics also mean that it
//! should be useable even in performance critical environments.

pub mod jiffy;

/// This Error is potentially returned by the Enqueue Operation
#[derive(Debug, PartialEq)]
pub enum EnqueueError {
    /// The Queue has been closed by the Consumer and therefore no more
    /// Elements should be enqueued on the Queue as they would never be
    /// consumed
    Closed,
}

/// This Error is potentially returned by the Enqueue Operation
#[derive(Debug, PartialEq)]
pub enum DequeueError {
    /// The Queue is most likely empty and therefore there is nothing to
    /// load right now.
    ///
    /// This indicates that the Operation could succeed in the Future if you
    /// attempt to perform it again
    WouldBlock,
    /// The Queue has been closed by the Producers and there will be no more
    /// Elements that could be dequeued from the Queue
    Closed,
}
