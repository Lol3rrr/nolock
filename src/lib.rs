#![deny(missing_docs)]
#![warn(rust_2018_idioms, missing_debug_implementations)]
//! This crate provides a set of Lock-Free algorithms and Datastructures
//!
//! # Features
//! * `hash_trie`: Enables the Hash-Trie-Map implementation
//! * `hazard_ptr`: Enables the Hazard-Ptr implementation
//! * `queues`: Enables all the Queues
//! * `async`: Enables all the Async-Version of the Algorithms/Datastructures
//! * `full`: Enables all the Feature-Flags

#[cfg(feature = "hash_trie")]
pub mod hash_trie;
#[cfg(feature = "hazard_ptr")]
pub mod hazard_ptr;
#[cfg(feature = "queues")]
pub mod queues;
