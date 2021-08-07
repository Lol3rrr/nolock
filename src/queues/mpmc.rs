//! Provides  Multi-Producer-Multi-Consumer Queues
//!
//! # Reference
//! * [A Scalable, Portable, and Memory-Efficient Lock-Free FIFO Queue](https://arxiv.org/pdf/1908.04511.pdf)

mod queue;

// TODO
// * Add the Unbounded version

pub mod bounded {
    //! This contains the Collection of bounded-MPMC-Queues proposed in [the Paper](https://arxiv.org/pdf/1908.04511.pdf),
    //! however you should basically always use [`scq`] over [`ncq`] as it scales better and in
    //! general is the intended implementation.
    //!
    //! # Example
    //! ```rust
    //! # use nolock::queues::mpmc::bounded;
    //! // Creates a new Queue with the Capacity for 10 Elements
    //! let (rx, tx) = bounded::scq::queue::<u64>(10);
    //!
    //! // Insert a new Element into the Queue
    //! assert_eq!(Ok(()), tx.try_enqueue(123));
    //! // Dequeue the Element again
    //! assert_eq!(Ok(123), rx.try_dequeue());
    //! ```

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
            /// # Example
            /// ## Valid/Normal enqueue
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::ncq;
            /// let (rx, tx) = ncq::queue::<u64>(10);
            ///
            /// assert_eq!(Ok(()), tx.try_enqueue(13));
            /// # drop(rx);
            /// ```
            ///
            /// ## Queue is already full
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::ncq;
            /// # use nolock::queues::EnqueueError;
            /// let (rx, tx) = ncq::queue::<u64>(1);
            /// // Enqueue an Element to fill the Queue
            /// tx.try_enqueue(13);
            ///
            /// assert_eq!(Err((EnqueueError::Full, 13)), tx.try_enqueue(13));
            /// # drop(rx);
            /// ```
            pub fn try_enqueue(&self, data: T) -> Result<(), (EnqueueError, T)> {
                self.0.try_enqueue(data)
            }

            /// Checks if the Receiving Half has closed the Queue, meaning that
            /// no more Elements would be dequeued from the Queue and therefore
            /// also should not be inserted anymore.
            ///
            /// # Example
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::ncq;
            /// let (rx, tx) = ncq::queue::<u64>(10);
            ///
            /// assert_eq!(false, tx.is_closed());
            ///
            /// drop(rx);
            ///
            /// assert_eq!(true, tx.is_closed());
            /// ```
            pub fn is_closed(&self) -> bool {
                self.0.is_closed()
            }
        }

        impl<T> Receiver<T> {
            /// Attempts to dequeue an Item from the Queue
            ///
            /// # Example
            /// ## Successfully enqueue Element
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::ncq;
            /// let (rx, tx) = ncq::queue::<u64>(10);
            ///
            /// // Enqueue an Item
            /// tx.try_enqueue(13).unwrap();
            ///
            /// // Dequeue the Item
            /// assert_eq!(Ok(13), rx.try_dequeue());
            /// ```
            ///
            /// ## Enqueue from empty Queue
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::ncq;
            /// # use nolock::queues::DequeueError;
            /// let (rx, tx) = ncq::queue::<u64>(10);
            ///
            /// // Attempt to Dequeue an item
            /// assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
            /// # drop(tx);
            /// ```
            pub fn try_dequeue(&self) -> Result<T, DequeueError> {
                self.0.dequeue()
            }

            /// Checks if the Sending Half has closed the Queue, meaning that
            /// no more new Elements will be added to the Queue.
            ///
            /// # Note
            /// Even if this indicates that the Queue has been closed, by the
            /// Sender and no more new Elements will be inserted into the Queue,
            /// there might still be Elements left in the Queue that are waiting
            /// to be dequeued.
            ///
            /// # Example
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::ncq;
            /// let (rx, tx) = ncq::queue::<u64>(10);
            ///
            /// assert_eq!(false, rx.is_closed());
            ///
            /// tx.try_enqueue(13).unwrap();
            /// drop(tx);
            ///
            /// assert_eq!(true, rx.is_closed());
            /// ```
            pub fn is_closed(&self) -> bool {
                self.0.is_closed()
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
            /// Attempts to Enqueue the given Data
            ///
            /// # Example
            /// ## Valid/Normal enqueue
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::scq;
            /// let (rx, tx) = scq::queue::<u64>(10);
            ///
            /// assert_eq!(Ok(()), tx.try_enqueue(13));
            /// # drop(rx);
            /// ```
            ///
            /// ## Queue is already full
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::scq;
            /// # use nolock::queues::EnqueueError;
            /// let (rx, tx) = scq::queue::<u64>(1);
            /// // Enqueue an Element to fill the Queue
            /// tx.try_enqueue(13);
            ///
            /// assert_eq!(Err((EnqueueError::Full, 13)), tx.try_enqueue(13));
            /// # drop(rx);
            /// ```
            pub fn try_enqueue(&self, data: T) -> Result<(), (EnqueueError, T)> {
                self.0.try_enqueue(data)
            }

            /// Checks if the Receiving Half has closed the Queue, meaning that
            /// no more Elements would be dequeued from the Queue and therefore
            /// also should not be inserted anymore.
            ///
            /// # Example
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::scq;
            /// let (rx, tx) = scq::queue::<u64>(10);
            ///
            /// assert_eq!(false, tx.is_closed());
            ///
            /// drop(rx);
            ///
            /// assert_eq!(true, tx.is_closed());
            /// ```
            pub fn is_closed(&self) -> bool {
                self.0.is_closed()
            }
        }

        impl<T> Receiver<T> {
            /// Attempts to Dequeue an item from the Queue
            ///
            /// # Example
            /// ## Successfully enqueue Element
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::scq;
            /// let (rx, tx) = scq::queue::<u64>(10);
            ///
            /// // Enqueue an Item
            /// tx.try_enqueue(13).unwrap();
            ///
            /// // Dequeue the Item
            /// assert_eq!(Ok(13), rx.try_dequeue());
            /// ```
            ///
            /// ## Enqueue from empty Queue
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::scq;
            /// # use nolock::queues::DequeueError;
            /// let (rx, tx) = scq::queue::<u64>(10);
            ///
            /// // Attempt to Dequeue an item
            /// assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
            /// # drop(tx);
            /// ```
            pub fn try_dequeue(&self) -> Result<T, DequeueError> {
                self.0.dequeue()
            }

            /// Checks if the Sending Half has closed the Queue, meaning that
            /// no more new Elements will be added to the Queue.
            ///
            /// # Note
            /// Even if this indicates that the Queue has been closed, by the
            /// Sender and no more new Elements will be inserted into the Queue,
            /// there might still be Elements left in the Queue that are waiting
            /// to be dequeued.
            ///
            /// # Example
            /// ```rust
            /// # use nolock::queues::mpmc::bounded::scq;
            /// let (rx, tx) = scq::queue::<u64>(10);
            ///
            /// assert_eq!(false, rx.is_closed());
            ///
            /// tx.try_enqueue(13).unwrap();
            /// drop(tx);
            ///
            /// assert_eq!(true, rx.is_closed());
            /// ```
            pub fn is_closed(&self) -> bool {
                self.0.is_closed()
            }
        }
    }
}
