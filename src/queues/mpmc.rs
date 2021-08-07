//! Provides  Multi-Producer-Multi-Consumer Queues
//!
//! # Reference
//! * [A Scalable, Portable, and Memory-Efficient Lock-Free FIFO Queue](https://arxiv.org/pdf/1908.04511.pdf)

mod queue;

// TODO
// * Add Support for detecting if a Queue has been closed by either side and then return the
// corresponding errors for further operations
// * Add the Unbounded version

pub mod bounded {
    //! This contains the Collection of bounded-MPMC-Queues proposed in [the Paper](https://arxiv.org/pdf/1908.04511.pdf),
    //! however you should basically always use [`scq`] over [`ncq`] as it scales better and in
    //! general is the intended implementation.
    use super::queue;

    pub mod ncq {
        //! This Queue uses the Naive-Circular-Queue implementation provided in [the Paper](https://arxiv.org/pdf/1908.04511.pdf).
        //!
        //! This is mostly here for completness, but in all basically all real-world cases, you
        //! should use the [`scq`](super::scq)-based-Queue, as that one scales better with more
        //! producers/consumers
        //!
        //! # Example:
        //! ```rust
        //! # use nolock::queues::mpmc::bounded::ncq;
        //! // Create the Queue
        //! let (rx, tx) = ncq::queue::<u64>(10);
        //!
        //! // Insert an Item into the Queue
        //! assert_eq!(Ok(()), tx.try_enqueue(10));
        //! // Dequeue the previously inserted Item
        //! assert_eq!(Ok(10), rx.try_dequeue());
        //! ```

        use std::fmt::Debug;

        use crate::queues::{DequeueError, EnqueueError};

        use super::queue;

        /// The receiving Half for a NCQ based MPMC-Queue
        pub struct Receiver<T>(queue::BoundedReceiver<T, queue::ncq::Queue>);
        /// The sending Half for a NCQ based MPMC-Queue
        pub struct Sender<T>(queue::BoundedSender<T, queue::ncq::Queue>);

        impl<T> Debug for Receiver<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // TODO
                write!(f, "NCQ-Receiver<{}>()", std::any::type_name::<T>())
            }
        }
        impl<T> Debug for Sender<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // TODO
                write!(f, "NCQ-Sender<{}>()", std::any::type_name::<T>())
            }
        }

        /// Creates a new NCQ-Queue with the given Capacity
        pub fn queue<T>(capacity: usize) -> (Receiver<T>, Sender<T>) {
            let (rx, tx) = queue::queue_ncq(capacity);
            (Receiver(rx), Sender(tx))
        }

        impl<T> Sender<T> {
            /// Attempts to enqueue the Data on the Queue
            ///
            /// # Returns
            /// * `Ok(())` if the Data was successfully enqueued
            /// * `Err(data)` if the Queue was full at the Time of enqueuing the Data
            pub fn try_enqueue(&self, data: T) -> Result<(), (EnqueueError, T)> {
                self.0.try_enqueue(data)
            }
        }

        impl<T> Receiver<T> {
            /// Attempts to dequeue an Item from the Queue
            ///
            /// # Returns
            /// * `Some(data)` if there was an Item to dequeue
            /// * `None` if there was no Item to dequeue at the time of dequeuing
            pub fn try_dequeue(&self) -> Result<T, DequeueError> {
                self.0.dequeue()
            }
        }
    }

    pub mod scq {
        //! This Queue uses the Scalable-Circular-Queue implementation provided in [the Paper](https://arxiv.org/pdf/1908.04511.pdf).
        //!
        //! # Example:
        //! ```rust
        //! # use nolock::queues::mpmc::bounded::scq;
        //! // Create the Queue
        //! let (rx, tx) = scq::queue::<u64>(10);
        //!
        //! // Insert an Item into the Queue
        //! assert_eq!(Ok(()), tx.try_enqueue(10));
        //! // Dequeue the previously inserted Item
        //! assert_eq!(Ok(10), rx.try_dequeue());
        //! ```

        use std::fmt::Debug;

        use crate::queues::{DequeueError, EnqueueError};

        use super::queue;

        /// The receiving Half for a SCQ based MPMC-Queue
        pub struct Receiver<T>(queue::BoundedReceiver<T, queue::scq::Queue>);
        /// The sending Half for a SCQ based MPMC-Queue
        pub struct Sender<T>(queue::BoundedSender<T, queue::scq::Queue>);

        impl<T> Debug for Receiver<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // TODO
                write!(f, "SCQ-Receiver<{}>()", std::any::type_name::<T>())
            }
        }
        impl<T> Debug for Sender<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // TODO
                write!(f, "SCQ-Sender<{}>()", std::any::type_name::<T>())
            }
        }

        /// Creates a new Queue with the given Capacity.
        ///
        /// Unlike the other Queues in this crate, this Queue combines the Producer and Consumer in
        /// a single Struct, as they dont have any restrictions that would limit the other half in
        /// some way and need to share certain state anyway.
        pub fn queue<T>(capacity: usize) -> (Receiver<T>, Sender<T>) {
            let (rx, tx) = queue::queue_scq(capacity);
            (Receiver(rx), Sender(tx))
        }

        impl<T> Sender<T> {
            /// Attempts to Enqueue the given Data.
            ///
            /// # Returns
            /// * `Ok(())` if the Data was successfully enqueued
            /// * `Err(data)` if the Queue is full and the Data could not be enqueued
            pub fn try_enqueue(&self, data: T) -> Result<(), (EnqueueError, T)> {
                self.0.try_enqueue(data)
            }
        }

        impl<T> Receiver<T> {
            /// Attempts to Dequeue an item from the Queue
            ///
            /// # Returns
            /// * `Some(item)` if there was an Item to dequeue
            /// * `None` if the Qeuue was empty at the Time of dequeuing
            pub fn try_dequeue(&self) -> Result<T, DequeueError> {
                self.0.dequeue()
            }
        }
    }
}
