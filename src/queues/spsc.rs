//! The SPSC-Queue is a Single-Producer Single-Consumer Queue

pub mod bounded {
    //! This implements a bounded lock-free Queue
    //!
    //! # Reference:
    //! * [FastForward for Efficient Pipeline Parallelism - A Cache-Optimized Concurrent Lock-Free Queue](https://www.researchgate.net/publication/213894711_FastForward_for_Efficient_Pipeline_Parallelism_A_Cache-Optimized_Concurrent_Lock-Free_Queue)

    use std::sync::{atomic, Arc};

    /// The Sending-Half for the queue
    pub struct BoundedSender<T> {
        head: usize,
        buffer: Arc<Vec<atomic::AtomicPtr<T>>>,
    }

    /// The Receiving-Half for the Queue
    pub struct BoundedReceiver<T> {
        tail: usize,
        buffer: Arc<Vec<atomic::AtomicPtr<T>>>,
    }

    /// The Error for the Enqueue Operation
    #[derive(Debug, PartialEq)]
    pub enum EnqueueError {
        /// This means that the Queue is full and the Element could not be
        /// inserted in this Moment
        WouldBlock,
    }

    fn next_element(current: usize, length: usize) -> usize {
        (current + 1) % length
    }

    impl<T> BoundedSender<T> {
        /// Attempts to Enqueue the given piece of Data
        pub fn try_enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
            if !self.buffer[self.head]
                .load(atomic::Ordering::SeqCst)
                .is_null()
            {
                return Err((data, EnqueueError::WouldBlock));
            }

            self.buffer[self.head].store(Box::into_raw(Box::new(data)), atomic::Ordering::SeqCst);

            Ok(())
        }

        /// Checks if the current Queue is full
        pub fn is_full(&self) -> bool {
            !self.buffer[self.head]
                .load(atomic::Ordering::SeqCst)
                .is_null()
        }
    }

    /// The Error for the Dequeue Operation
    #[derive(Debug, PartialEq)]
    pub enum DequeueError {
        /// This indicates that no Data could be dequeued
        WouldBlock,
    }

    impl<T> BoundedReceiver<T> {
        /// Attempts to Dequeue the given piece of Data
        pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
            let data_ptr = self.buffer[self.tail].load(atomic::Ordering::SeqCst);
            if data_ptr.is_null() {
                return Err(DequeueError::WouldBlock);
            }

            self.buffer[self.tail].store(0 as *mut T, atomic::Ordering::SeqCst);
            self.tail = next_element(self.tail, self.buffer.len());

            let boxed_data = unsafe { Box::from_raw(data_ptr) };

            Ok(*boxed_data)
        }

        /// Checks if the current queue is empty
        pub fn is_empty(&self) -> bool {
            self.buffer[self.tail]
                .load(atomic::Ordering::SeqCst)
                .is_null()
        }
    }

    /// Creates a new Bounded-Queue with the given Size
    pub fn bounded_queue<T>(size: usize) -> (BoundedReceiver<T>, BoundedSender<T>) {
        let mut raw_buffer = Vec::with_capacity(size);
        for _ in 0..size {
            raw_buffer.push(atomic::AtomicPtr::new(0 as *mut T));
        }

        let buffer = Arc::new(raw_buffer);

        (
            BoundedReceiver {
                buffer: buffer.clone(),
                tail: 0,
            },
            BoundedSender { buffer, head: 0 },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn enqueue_dequeue() {
            let (mut rx, mut tx) = bounded_queue(10);

            assert_eq!(Ok(()), tx.try_enqueue(13));
            assert_eq!(Ok(13), rx.try_dequeue());
        }
        #[test]
        fn enqueue_will_block() {
            let (_, mut tx) = bounded_queue(1);

            assert_eq!(Ok(()), tx.try_enqueue(13));
            assert_eq!(Err((14, EnqueueError::WouldBlock)), tx.try_enqueue(14));
        }
        #[test]
        fn dequeue_will_block() {
            let (mut rx, _) = bounded_queue::<usize>(1);

            assert_eq!(Err(DequeueError::WouldBlock), rx.try_dequeue());
        }
    }
}

pub mod unbounded {
    //! An unbounded lock-free Queue
    //!
    //! # Reference:
    //! * [An Efficient Unbounded Lock-Free Queue - for Multi-core Systems](https://link.springer.com/content/pdf/10.1007%2F978-3-642-32820-6_65.pdf)

    pub mod dSPSC {
        //! The Basic and slower version

        use super::super::bounded;

        use std::sync::atomic;

        /// The Node datastructure used for the unbounded Queue
        pub struct Node<T> {
            data: Option<T>,
            next: atomic::AtomicPtr<Node<T>>,
        }

        /// The Unbounded Sender Half
        pub struct UnboundedSender<T> {
            tail: *mut Node<T>,
            node_receiver: bounded::BoundedReceiver<Box<Node<T>>>,
        }

        impl<T> UnboundedSender<T> {
            /// Enqueues the given Data
            pub fn enqueue(&mut self, data: T) {
                let node = match self.node_receiver.try_dequeue() {
                    Ok(mut n) => {
                        n.data = Some(data);
                        n.next.store(0 as *mut Node<T>, atomic::Ordering::SeqCst);
                        n
                    }
                    Err(_) => Box::new(Node {
                        data: Some(data),
                        next: atomic::AtomicPtr::new(0 as *mut Node<T>),
                    }),
                };

                let node_ptr = Box::into_raw(node);
                let tail_boxed = unsafe { Box::from_raw(self.tail) };
                tail_boxed.next.store(node_ptr, atomic::Ordering::SeqCst);
                self.tail = node_ptr;

                // We dont want to free any memory in the Enqueue operation
                std::mem::forget(tail_boxed);
            }
        }

        /// The Unbounded Receiver Half
        pub struct UnboundedReceiver<T> {
            head: *mut Node<T>,
            node_return: bounded::BoundedSender<Box<Node<T>>>,
        }

        impl<T> UnboundedReceiver<T> {
            /// Attempts to dequeue a piece of Data
            pub fn try_dequeue(&mut self) -> Result<T, ()> {
                let prev_head = unsafe { Box::from_raw(self.head) };
                let next_ptr = prev_head.next.load(atomic::Ordering::SeqCst);
                if next_ptr.is_null() {
                    std::mem::forget(prev_head);
                    return Err(());
                }

                let mut next = unsafe { Box::from_raw(next_ptr) };
                let data = next.data.take().unwrap();

                self.head = next_ptr;
                std::mem::forget(next);
                if let Err((node, _)) = self.node_return.try_enqueue(prev_head) {
                    drop(node);
                }

                Ok(data)
            }

            /// Checks if the current Queue is empty
            pub fn is_empty(&mut self) -> bool {
                let prev_head = unsafe { Box::from_raw(self.head) };
                let next_ptr = prev_head.next.load(atomic::Ordering::SeqCst);
                std::mem::forget(prev_head);

                next_ptr.is_null()
            }
        }

        /// Creates a new Unbounded Queue-Pair
        pub fn unbounded_basic_queue<T>() -> (UnboundedReceiver<T>, UnboundedSender<T>) {
            let (node_rx, node_tx) = bounded::bounded_queue(64);
            let dummy_node = Box::new(Node {
                data: None,
                next: atomic::AtomicPtr::new(0 as *mut Node<T>),
            });
            let dummy_ptr = Box::into_raw(dummy_node);

            (
                UnboundedReceiver {
                    head: dummy_ptr,
                    node_return: node_tx,
                },
                UnboundedSender {
                    tail: dummy_ptr,
                    node_receiver: node_rx,
                },
            )
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn enqueue_dequeue() {
                let (mut rx, mut tx) = unbounded_basic_queue();

                tx.enqueue(13);
                assert_eq!(Ok(13), rx.try_dequeue());
            }
            #[test]
            fn dequeue_empty() {
                let (mut rx, mut tx) = unbounded_basic_queue();

                assert_eq!(Err(()), rx.try_dequeue());
                tx.enqueue(13);
                assert_eq!(Ok(13), rx.try_dequeue());
                assert_eq!(Err(()), rx.try_dequeue());
            }
            #[test]
            fn multiple_enqueue_dequeue() {
                let (mut rx, mut tx) = unbounded_basic_queue();

                tx.enqueue(13);
                assert_eq!(Ok(13), rx.try_dequeue());
                tx.enqueue(14);
                assert_eq!(Ok(14), rx.try_dequeue());
                tx.enqueue(15);
                assert_eq!(Ok(15), rx.try_dequeue());
            }
            #[test]
            fn multiple_enqueue_dequeue_2() {
                let (mut rx, mut tx) = unbounded_basic_queue();

                tx.enqueue(13);
                tx.enqueue(14);
                tx.enqueue(15);
                assert_eq!(Ok(13), rx.try_dequeue());
                assert_eq!(Ok(14), rx.try_dequeue());
                assert_eq!(Ok(15), rx.try_dequeue());
            }
        }
    }

    use super::bounded;

    // TODO
    // Add Support for the Caches to improve the Performance and reduce the overhead
    // of the Allocator

    /// The Sender-Half of an unbounded Queue
    pub struct UnboundedSender<T> {
        buffer_size: usize,
        buf_w: bounded::BoundedSender<T>,
        inuse_sender: dSPSC::UnboundedSender<bounded::BoundedReceiver<T>>,
    }

    impl<T> UnboundedSender<T> {
        fn next_w(&mut self) -> bounded::BoundedSender<T> {
            let (rx, tx) = bounded::bounded_queue(self.buffer_size);
            self.inuse_sender.enqueue(rx);
            tx
        }

        /// Enqueues the Data
        pub fn enqueue(&mut self, data: T) {
            if self.buf_w.is_full() {
                self.buf_w = self.next_w();
            }
            if let Err(_) = self.buf_w.try_enqueue(data) {
                panic!("The new Buffer should always have capacity for a new Element");
            }
        }
    }

    /// The Receiver-Half of an unbounded Queue
    pub struct UnboundedReceiver<T> {
        buf_r: bounded::BoundedReceiver<T>,
        inuse_recv: dSPSC::UnboundedReceiver<bounded::BoundedReceiver<T>>,
    }

    impl<T> UnboundedReceiver<T> {
        fn next_r(&mut self) -> Option<bounded::BoundedReceiver<T>> {
            match self.inuse_recv.try_dequeue() {
                Ok(b) => Some(b),
                Err(_) => None,
            }
        }

        /// Attempts to dequeue a single Element from the Queue
        pub fn dequeue(&mut self) -> Result<T, ()> {
            if self.buf_r.is_empty() {
                if self.inuse_recv.is_empty() {
                    return Err(());
                }
                if self.buf_r.is_empty() {
                    self.buf_r = match self.next_r() {
                        Some(b) => b,
                        None => return Err(()),
                    };
                }
            }

            self.buf_r.try_dequeue().map_err(|_| ())
        }
    }

    /// Creates a new Queue
    pub fn unbounded_queue<T>(buffer_size: usize) -> (UnboundedReceiver<T>, UnboundedSender<T>) {
        let (inuse_rx, inuse_tx) = dSPSC::unbounded_basic_queue();
        let (initial_rx, initial_tx) = bounded::bounded_queue(buffer_size);

        (
            UnboundedReceiver {
                buf_r: initial_rx,
                inuse_recv: inuse_rx,
            },
            UnboundedSender {
                buffer_size,
                buf_w: initial_tx,
                inuse_sender: inuse_tx,
            },
        )
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn enqueue_dequeue() {
            let (mut rx, mut tx) = unbounded_queue(10);

            tx.enqueue(13);
            assert_eq!(Ok(13), rx.dequeue());
        }

        #[test]
        fn multi_buffer() {
            let (mut rx, mut tx) = unbounded_queue(1);

            tx.enqueue(13);
            tx.enqueue(14);
            tx.enqueue(15);

            assert_eq!(Ok(13), rx.dequeue());
            assert_eq!(Ok(14), rx.dequeue());
            assert_eq!(Ok(15), rx.dequeue());
        }
    }
}
