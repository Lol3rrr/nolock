//! The SPSC-Queue is a Single-Producer Single-Consumer Queue
//!
//! # Bounded
//! The Bounded-Queue is currently the fastest Queue available in this crate
//! and should be the preferred Queue to use. However the major drawback is the
//! fact, that it is bounded and can therefore only hold a limited number of
//! items at a time.
//!
//! # Unbounded
//! The Unbounded-Queue is still really fast and will most likely be fast
//! enough for most use-cases, however since this Queue is unbounded it has
//! a broader range of applications as it can "grow" as needed without
//! having to sacrifice a lot of performance.

pub mod bounded;

pub mod unbounded;
