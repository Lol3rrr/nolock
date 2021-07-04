//! The SPSC-Queue is a Single-Producer Single-Consumer Queue

/// The Error for the Dequeue Operation
#[derive(Debug, PartialEq)]
pub enum DequeueError {
    /// This indicates that no Data could be dequeued
    WouldBlock,
    /// This indicates that the Queue is empty and the other side of the
    /// Queue has been dropped, meaning that no more Items can be added to the
    /// Queue either
    Closed,
}

/// The Error for the Enqueue Operation
#[derive(Debug, PartialEq)]
pub enum EnqueueError {
    /// This means that the Queue is full and the Element could not be
    /// inserted in this Moment
    WouldBlock,
    /// This indicates that the Queue is empty and the other side of the
    /// Queue has been dropped, meaning that no more Items can be added to the
    /// Queue either
    Closed,
}

pub mod bounded;

pub mod unbounded;
