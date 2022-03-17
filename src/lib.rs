#![cfg_attr(not(feature = "std"), no_std)]
#![feature(doc_cfg)]
#![deny(missing_docs, unsafe_op_in_unsafe_fn)]
#![warn(rust_2018_idioms, missing_debug_implementations)]
//! This crate provides a set of Lock-Free algorithms and Datastructures
//!
//! # Feature-Flags
//! * `queues`: Enables all the Queues
//! * `async`: Enables all the Async-Version of the Algorithms/Datastructures
//! * `thread_data`: Enables the ThreadData Module
//! * `hazard_ptr`: Enables the Hazard-Ptr implementation
//! * `allocator`: Enables the Allocators
//! * `full`: Enables all the Feature-Flags
//!
//! # Experimental-Feature-Flags
//! * `hash_trie`: Enables the Hash-Trie-Map implementation

extern crate alloc;

#[cfg(feature = "allocator")]
#[doc(cfg(feature = "allocator"))]
pub mod allocator;
#[cfg(feature = "hash_trie")]
#[doc(cfg(feature = "hash_trie"))]
pub mod hash_trie;
#[cfg(feature = "hazard_ptr")]
#[doc(cfg(feature = "hazard_ptr"))]
pub mod hazard_ptr;
#[cfg(feature = "queues")]
#[doc(cfg(feature = "queues"))]
pub mod queues;
#[cfg(feature = "thread_data")]
#[doc(cfg(feature = "thread_data"))]
pub mod thread_data;
