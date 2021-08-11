#![deny(missing_docs)]
#![warn(rust_2018_idioms, missing_debug_implementations)]
//! This crate provides a set of Lock-Free algorithms and Datastructures
//!
//! # Feature-Flags
//! * `queues`: Enables all the Queues
//! * `async`: Enables all the Async-Version of the Algorithms/Datastructures
//! * `thread_data`: Enables the ThreadData Module
//! * `hazard_ptr`: Enables the Hazard-Ptr implementation
//! * `full`: Enables all the Feature-Flags
//!
//! # Experimental-Feature-Flags
//! * `hash_trie`: Enables the Hash-Trie-Map implementation

#[cfg(feature = "hash_trie")]
pub mod hash_trie;
#[cfg(feature = "hazard_ptr")]
pub mod hazard_ptr;
#[cfg(feature = "queues")]
pub mod queues;
#[cfg(feature = "thread_data")]
pub mod thread_data;
