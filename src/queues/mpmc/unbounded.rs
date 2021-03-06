//! This contains the unbounded Version of the MPMC Queue proposed in [the Paper]((https://arxiv.org/pdf/1908.04511.pdf))
//!
//! # Example
//! ```rust
//! # use nolock::queues::mpmc::unbounded;
//! let (rx, tx) = unbounded::queue::<u64>();
//!
//! assert_eq!(Ok(()), tx.enqueue(13));
//! assert_eq!(Ok(13), rx.try_dequeue());
//! ```

use crate::sync::atomic;
use std::{fmt::Debug, sync::Arc};

use crate::{hyaline, queues::DequeueError};

mod async_queue;
pub use async_queue::{async_queue, AsyncReceiver, AsyncSender};

mod queue;

const BUFFER_SIZE: usize = 128;

/// The Receiver Half of an unbounded LSCQ Queue
pub struct Receiver<T> {
    head: atomic::AtomicPtr<queue::BoundedQueue<T>>,
    rx_count: Arc<atomic::AtomicU64>,
    tx_count: Arc<atomic::AtomicU64>,
    hyaline_instance: Arc<hyaline::Hyaline>,
}
/// The Sender Half of an unbounded LSCQ Queue
pub struct Sender<T> {
    tail: atomic::AtomicPtr<queue::BoundedQueue<T>>,
    rx_count: Arc<atomic::AtomicU64>,
    tx_count: Arc<atomic::AtomicU64>,
    hyaline_instance: Arc<hyaline::Hyaline>,
}

impl<T> Debug for Receiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LSCQ-Receiver<{}>", std::any::type_name::<T>())
    }
}
impl<T> Debug for Sender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LSCQ-Sender<{}>", std::any::type_name::<T>())
    }
}

fn free_fn<T>(ptr: *const ()) {
    let boxed = unsafe { Box::from_raw(ptr as *mut queue::BoundedQueue<T>) };
    drop(boxed);
}

/// Creates a new unbounded LSCQ Queue
pub fn queue<T>() -> (Receiver<T>, Sender<T>) {
    let initial_buffer = Box::new(queue::new_queue(BUFFER_SIZE));
    let initial_buffer_ptr = Box::into_raw(initial_buffer);

    let head = atomic::AtomicPtr::new(initial_buffer_ptr);
    let tail = atomic::AtomicPtr::new(initial_buffer_ptr);

    let rx_count = Arc::new(atomic::AtomicU64::new(1));
    let tx_count = Arc::new(atomic::AtomicU64::new(1));

    let instance = Arc::new(hyaline::Hyaline::new(free_fn::<T>));

    let rx = Receiver {
        head,
        rx_count: rx_count.clone(),
        tx_count: tx_count.clone(),
        hyaline_instance: instance.clone(),
    };
    let tx = Sender {
        tail,
        rx_count,
        tx_count,
        hyaline_instance: instance,
    };

    (rx, tx)
}

impl<T> Sender<T> {
    /// Attempts to enqueue the given Data on the Queue, this will only fail
    /// if the Queue has been closed by all the Receivers because then no one
    /// would be able to Dequeue the Data again.
    ///
    /// # Example
    /// ```rust
    /// # use nolock::queues::mpmc::unbounded;
    /// let (rx, tx) = unbounded::queue::<usize>();
    ///
    /// assert_eq!(Ok(()), tx.enqueue(123));
    ///
    /// # drop(rx);
    /// ```
    pub fn enqueue(&self, mut data: T) -> Result<(), T> {
        let handle = self.hyaline_instance.enter();

        loop {
            let tail_ptr = self.tail.load(atomic::Ordering::Acquire);
            let tail = unsafe { &*tail_ptr };

            if self.is_closed() {
                return Err(data);
            }

            let next_ptr = tail.next.load(atomic::Ordering::Acquire);
            if !next_ptr.is_null() {
                let _ = self.tail.compare_exchange(
                    tail_ptr,
                    next_ptr,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                );
                continue;
            }

            data = match tail.try_enqueue(data) {
                Ok(_) => return Ok(()),
                Err((_, d)) => d,
            };

            let (n_queue_ptr, n_queue) = {
                let raw = Box::new(queue::new_queue(BUFFER_SIZE));
                let raw_ptr = Box::into_raw(raw);

                let raw_ref = unsafe { &*raw_ptr };
                (raw_ptr, raw_ref)
            };
            let _ = n_queue.try_enqueue(data);

            match tail.next.compare_exchange(
                std::ptr::null_mut(),
                n_queue_ptr,
                atomic::Ordering::AcqRel,
                atomic::Ordering::Relaxed,
            ) {
                Ok(_) => {
                    let _ = self.tail.compare_exchange(
                        tail_ptr,
                        n_queue_ptr,
                        atomic::Ordering::AcqRel,
                        atomic::Ordering::Relaxed,
                    );

                    drop(handle);
                    return Ok(());
                }
                Err(_) => {
                    data = n_queue.dequeue().expect("");

                    drop(unsafe { Box::from_raw(n_queue_ptr) });
                }
            };
        }
    }

    /// Checks if the Queue has been closed by the Receiver Side
    ///
    /// # Example
    /// ```rust
    /// # use nolock::queues::mpmc::unbounded;
    /// let (rx, tx) = unbounded::queue::<usize>();
    ///
    /// // Queue is still open
    /// assert_eq!(false, tx.is_closed());
    ///
    /// // Close from the Receiving Side by dropping the Receiver
    /// drop(rx);
    ///
    /// assert_eq!(true, tx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.rx_count.load(atomic::Ordering::Acquire) == 0
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.tx_count.fetch_sub(1, atomic::Ordering::AcqRel);
    }
}

impl<T> Receiver<T> {
    /// Attempts to Dequeue an Entry from the Queue
    ///
    /// # Example
    /// ```rust
    /// # use nolock::queues::mpmc::unbounded;
    /// # use nolock::queues::DequeueError;
    /// let (rx, tx) = unbounded::queue::<usize>();
    ///
    /// tx.enqueue(13).unwrap();
    ///
    /// assert_eq!(Ok(13), rx.try_dequeue());
    /// assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
    /// ```
    pub fn try_dequeue(&self) -> Result<T, DequeueError> {
        let mut handle = self.hyaline_instance.enter();

        loop {
            if self.is_closed() {
                return Err(DequeueError::Closed);
            }

            let head_ptr = self.head.load(atomic::Ordering::Acquire);
            let head = unsafe { &*head_ptr };

            if let Ok(data) = head.dequeue() {
                return Ok(data);
            }

            let next_ptr = head.next.load(atomic::Ordering::Acquire);
            if next_ptr.is_null() {
                return Err(DequeueError::Empty);
            }

            let thres_chk = (head.aq.size * 3 - 1) as isize;
            head.aq
                .threshold
                .store(thres_chk, atomic::Ordering::Release);

            if let Ok(data) = head.dequeue() {
                return Ok(data);
            }

            if self
                .head
                .compare_exchange(
                    head_ptr,
                    next_ptr,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                // Safety:
                // This is safe to do, because the Pointer is no longer
                // accessible from a shared Reference/Pointer since we just
                // performed a successful compare_exchange on it.
                unsafe {
                    handle.retire(head_ptr as *const ());
                };
            }
        }
    }

    /// Checks if the Queue has been closed by the Sender Side
    ///
    /// # Note
    /// Even if the Queue is closed, there may still be Elements to dequeue.
    /// A Closed-Queue simply indicates that no more new Elements will be added
    /// but does not make any garantues about the current Content of the Queue
    ///
    /// # Example
    /// ```rust
    /// # use nolock::queues::mpmc::unbounded;
    /// let (rx, tx) = unbounded::queue::<usize>();
    ///
    /// assert_eq!(false, rx.is_closed());
    ///
    /// // Close the Queue by droping the Sender
    /// drop(tx);
    ///
    /// assert_eq!(true, rx.is_closed());
    /// ```
    pub fn is_closed(&self) -> bool {
        self.tx_count.load(atomic::Ordering::Acquire) == 0
    }
}
impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut handle = self.hyaline_instance.enter();

        self.rx_count.fetch_sub(1, atomic::Ordering::AcqRel);

        let mut current_ptr = self.head.load(atomic::Ordering::SeqCst);
        let mut current = unsafe { &*current_ptr };

        loop {
            let next_ptr = current.next.load(atomic::Ordering::SeqCst);

            // Safety:
            // This is safe to do, because we are the only Thread with access
            // to the Queue anymore or at least the Head of the Queue meaning
            // that no other Thread could load the Pointers anymore and
            // therefore we can savely retire the entire List
            unsafe {
                handle.retire(current_ptr as *const ());
            };

            if next_ptr.is_null() {
                break;
            }
            current_ptr = next_ptr;
            current = unsafe { &*current_ptr };
        }

        drop(handle);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_queue() {
        queue::<u64>();
    }

    #[test]
    fn enqueue() {
        let (rx, tx) = queue::<u64>();

        assert_eq!(Ok(()), tx.enqueue(13));
        drop(rx);
    }

    #[test]
    fn enqueue_dequeue() {
        let (rx, tx) = queue::<u64>();

        assert_eq!(Ok(()), tx.enqueue(13));
        assert_eq!(Ok(13), rx.try_dequeue());
    }

    #[test]
    fn enqueue_fill_multiple() {
        let (rx, tx) = queue::<usize>();

        for index in 0..(BUFFER_SIZE * 3) {
            tx.enqueue(index).unwrap();
        }
        drop(rx);
    }
    #[test]
    fn enqueue_dequeue_fill_multiple() {
        let (rx, tx) = queue::<usize>();

        for index in 0..(BUFFER_SIZE * 3) {
            tx.enqueue(index).unwrap();
            assert_eq!(Ok(index), rx.try_dequeue());
        }
    }
    #[test]
    fn enqueue_fill_multiple_dequeue_all() {
        let (rx, tx) = queue::<usize>();

        for index in 0..(BUFFER_SIZE * 3) {
            tx.enqueue(index).unwrap();
        }
        for index in 0..(BUFFER_SIZE * 3) {
            assert_eq!(Ok(index), rx.try_dequeue());
        }
    }

    #[test]
    fn receiver_is_closed() {
        let (rx, tx) = queue::<u64>();

        assert!(!rx.is_closed());

        drop(tx);
        assert!(rx.is_closed());
    }
    #[test]
    fn sender_is_closed() {
        let (rx, tx) = queue::<u64>();

        assert!(!tx.is_closed());

        drop(rx);
        assert!(tx.is_closed());
    }

    #[test]
    fn enqueue_on_closed() {
        let (rx, tx) = queue::<u64>();

        assert_eq!(Ok(()), tx.enqueue(13));
        drop(rx);

        assert_eq!(Err(14), tx.enqueue(14));
    }
    #[test]
    fn dequeue_on_closed() {
        let (rx, tx) = queue::<u64>();

        assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
        drop(tx);

        assert_eq!(Err(DequeueError::Closed), rx.try_dequeue());
    }
}
