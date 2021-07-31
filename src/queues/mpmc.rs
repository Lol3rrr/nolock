//! Provides  Multi-Producer-Multi-Consumer Queues
//!
//! # Reference
//! * [A Scalable, Portable, and Memory-Efficient Lock-Free FIFO Queue](https://arxiv.org/pdf/1908.04511.pdf)

mod queue;

pub mod bounded {
    //! TODO
    use super::queue;

    pub mod ncq {
        //! TODO
        use super::queue;

        /// TODO
        pub struct Queue<T>(queue::Bounded<T, queue::ncq::Queue>);

        /// TODO
        pub fn queue<T>(capacity: usize) -> Queue<T> {
            Queue(queue::Bounded::new_ncq(capacity))
        }

        impl<T> Queue<T> {
            /// TODO
            pub fn enqueue(&self, data: T) -> Result<(), T> {
                self.0.enqueue(data)
            }
            /// TODO
            pub fn try_dequeue(&self) -> Option<T> {
                self.0.dequeue()
            }
        }
    }

    pub mod scq {
        //! TODO
        use super::queue;

        /// TODO
        pub struct Queue<T>(queue::Bounded<T, queue::scq::Queue>);

        /// TODO
        pub fn queue<T>(capacity: usize) -> Queue<T> {
            Queue(queue::Bounded::new_scq(capacity))
        }

        impl<T> Queue<T> {
            /// TODO
            pub fn enqueue(&self, data: T) -> Result<(), T> {
                self.0.enqueue(data)
            }
            /// TODO
            pub fn try_dequeue(&self) -> Option<T> {
                self.0.dequeue()
            }
        }
    }
}
