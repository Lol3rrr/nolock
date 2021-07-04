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

pub mod mpsc;
pub mod spsc;
