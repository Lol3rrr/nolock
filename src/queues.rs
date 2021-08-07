//! This module provides a variety of different Queue implementations
//! that are useful for different use-cases
//!
//! # MPSC
//! These are queues that allow multiple Producers and one Consumer. The
//! consumers are allowed to concurrently insert elements into the Queue
//!
//! # SPSC
//! These are queues that have one Producer and one Consumer, these are useful
//! for having two different processes/threads/tasks communicate with each
//! other

/// The Error returned by the Enqueue Operation
#[derive(Debug, PartialEq)]
pub enum EnqueueError {
    /// The Queue is full and therefore the current Element could not be enqueued on it
    Full,
    /// The Queue has been closed by the Receiving Side and therefore they can't receive
    /// any more Elements / any Element that would be inserted at this point would never
    /// be consumed
    Closed,
}

/// The Error returned by the Dequeue Operation
#[derive(Debug, PartialEq)]
pub enum DequeueError {
    /// The Queue is empty and therefore no Element could be dequeued at this point in time
    Empty,
    /// The Queue has been closed by the Sending Side and therefore no more Elements will
    /// be added to the Queue in the Future
    Closed,
}

pub mod mpmc;
pub mod mpsc;
pub mod spsc;
