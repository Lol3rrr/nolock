//! Provides  Multi-Producer-Multi-Consumer Queues
//!
//! # Reference
//! * [A Scalable, Portable, and Memory-Efficient Lock-Free FIFO Queue](https://arxiv.org/pdf/1908.04511.pdf)

mod queue;

pub mod bounded;
#[cfg(feature = "hyaline")]
#[cfg_attr(docsrs, doc(cfg(feature = "hyaline")))]
pub mod unbounded;
