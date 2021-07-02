//! This implements a bounded lock-free Queue
//!
//! # Reference:
//! * [FastForward for Efficient Pipeline Parallelism - A Cache-Optimized Concurrent Lock-Free Queue](https://www.researchgate.net/publication/213894711_FastForward_for_Efficient_Pipeline_Parallelism_A_Cache-Optimized_Concurrent_Lock-Free_Queue)

use std::{
    fmt::Debug,
    sync::{atomic, Arc},
};

use super::{DequeueError, EnqueueError};

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

const fn next_element(current: usize, length: usize) -> usize {
    (current + 1) % length
}

impl<T> BoundedSender<T> {
    /// Attempts to Enqueue the given piece of Data
    pub fn try_enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
        if !self.buffer[self.head]
            .load(atomic::Ordering::Acquire)
            .is_null()
        {
            return Err((data, EnqueueError::WouldBlock));
        }

        self.buffer[self.head].store(Box::into_raw(Box::new(data)), atomic::Ordering::Release);
        self.head = next_element(self.head, self.buffer.len());

        Ok(())
    }

    /// Checks if the current Queue is full
    pub fn is_full(&self) -> bool {
        !self.buffer[self.head]
            .load(atomic::Ordering::Acquire)
            .is_null()
    }
}

impl<T> Debug for BoundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BoundedSender ()")
    }
}

impl<T> BoundedReceiver<T> {
    /// Attempts to Dequeue the given piece of Data
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        let buffer_entry = &self.buffer[self.tail];

        let data_ptr = buffer_entry.load(atomic::Ordering::Acquire);
        if data_ptr.is_null() {
            return Err(DequeueError::WouldBlock);
        }

        buffer_entry.store(std::ptr::null_mut(), atomic::Ordering::Release);
        self.tail = next_element(self.tail, self.buffer.len());

        let boxed_data = unsafe { Box::from_raw(data_ptr) };

        Ok(*boxed_data)
    }

    /// Checks if the current queue is empty
    pub fn is_empty(&self) -> bool {
        self.buffer[self.tail]
            .load(atomic::Ordering::Acquire)
            .is_null()
    }
}

impl<T> Debug for BoundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BoundedReceiver ()")
    }
}

/// Creates a new Bounded-Queue with the given Size
pub fn bounded_queue<T>(size: usize) -> (BoundedReceiver<T>, BoundedSender<T>) {
    let mut raw_buffer = Vec::with_capacity(size);
    for _ in 0..size {
        raw_buffer.push(atomic::AtomicPtr::new(std::ptr::null_mut()));
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

    #[test]
    fn enqueue_dequeue_full_buffer() {
        let (mut rx, mut tx) = bounded_queue(3);

        for i in 0..4 {
            assert_eq!(Ok(()), tx.try_enqueue(i));
            assert_eq!(Ok(i), rx.try_dequeue());
        }
    }
}
