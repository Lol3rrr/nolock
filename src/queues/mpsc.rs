//! This provides some Multi-Producer Single-Consumer queues

pub mod jiffy;

/// This Error is potentially returned by the Enqueue Operation
#[derive(Debug, PartialEq)]
pub enum EnqueueError {
    /// The Queue has been closed by the other side
    Closed,
}

/// This Error is potentially returned by the Enqueue Operation
#[derive(Debug, PartialEq)]
pub enum DequeueError {
    /// The Queue is most likely empty and therefore there is nothing to
    /// load right now
    WouldBlock,
    /// The Queue has been closed by the other side
    Closed,
}
