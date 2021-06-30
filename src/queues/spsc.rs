//! The SPSC-Queue is a Single-Producer Single-Consumer Queue

/// The Error for the Dequeue Operation
#[derive(Debug, PartialEq)]
pub enum DequeueError {
    /// This indicates that no Data could be dequeued
    WouldBlock,
}

/// The Error for the Enqueue Operation
#[derive(Debug, PartialEq)]
pub enum EnqueueError {
    /// This means that the Queue is full and the Element could not be
    /// inserted in this Moment
    WouldBlock,
}

pub mod bounded;

pub mod unbounded;
