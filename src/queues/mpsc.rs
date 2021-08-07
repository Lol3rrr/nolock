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
