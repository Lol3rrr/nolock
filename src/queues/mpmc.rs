//! Provides  Multi-Producer-Multi-Consumer Queues
//!
//! # Reference
//! * [A Scalable, Portable, and Memory-Efficient Lock-Free FIFO Queue](https://arxiv.org/pdf/1908.04511.pdf)

mod queue;

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
        //! let queue = ncq::queue::<u64>(10);
        //!
        //! // Insert an Item into the Queue
        //! assert_eq!(Ok(()), queue.try_enqueue(10));
        //! // Dequeue the previously inserted Item
        //! assert_eq!(Some(10), queue.try_dequeue());
        //! ```

        use std::fmt::Debug;

        use super::queue;

        /// The Consumer and Producer for the NCQ-Queue
        pub struct Queue<T>(queue::Bounded<T, queue::ncq::Queue>);

        impl<T> Debug for Queue<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // TODO
                write!(f, "NCQ-Queue<{}>()", std::any::type_name::<T>())
            }
        }

        /// Creates a new NCQ-Queue with the given Capacity
        pub fn queue<T>(capacity: usize) -> Queue<T> {
            Queue(queue::Bounded::new_ncq(capacity))
        }

        impl<T> Queue<T> {
            /// Attempts to enqueue the Data on the Queue
            ///
            /// # Returns
            /// * `Ok(())` if the Data was successfully enqueued
            /// * `Err(data)` if the Queue was full at the Time of enqueuing the Data
            pub fn try_enqueue(&self, data: T) -> Result<(), T> {
                self.0.try_enqueue(data)
            }

            /// Attempts to dequeue an Item from the Queue
            ///
            /// # Returns
            /// * `Some(data)` if there was an Item to dequeue
            /// * `None` if there was no Item to dequeue at the time of dequeuing
            pub fn try_dequeue(&self) -> Option<T> {
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
        //! let queue = scq::queue::<u64>(10);
        //!
        //! // Insert an Item into the Queue
        //! assert_eq!(Ok(()), queue.try_enqueue(10));
        //! // Dequeue the previously inserted Item
        //! assert_eq!(Some(10), queue.try_dequeue());
        //! ```

        use std::fmt::Debug;

        use super::queue;

        /// The Consumer and Producer for the SCQ-Queue
        pub struct Queue<T>(queue::Bounded<T, queue::scq::Queue>);

        impl<T> Debug for Queue<T> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // TODO
                write!(f, "SCQ-Queue<{}>()", std::any::type_name::<T>())
            }
        }

        /// Creates a new Queue with the given Capacity.
        ///
        /// Unlike the other Queues in this crate, this Queue combines the Producer and Consumer in
        /// a single Struct, as they dont have any restrictions that would limit the other half in
        /// some way and need to share certain state anyway.
        pub fn queue<T>(capacity: usize) -> Queue<T> {
            Queue(queue::Bounded::new_scq(capacity))
        }

        impl<T> Queue<T> {
            /// Attempts to Enqueue the given Data.
            ///
            /// # Returns
            /// * `Ok(())` if the Data was successfully enqueued
            /// * `Err(data)` if the Queue is full and the Data could not be enqueued
            pub fn try_enqueue(&self, data: T) -> Result<(), T> {
                self.0.try_enqueue(data)
            }

            /// Attempts to Dequeue an item from the Queue
            ///
            /// # Returns
            /// * `Some(item)` if there was an Item to dequeue
            /// * `None` if the Qeuue was empty at the Time of dequeuing
            pub fn try_dequeue(&self) -> Option<T> {
                self.0.dequeue()
            }
        }
    }
}
