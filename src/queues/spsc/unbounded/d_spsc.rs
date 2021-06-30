//! The Basic and slower version

use super::super::{bounded, DequeueError};

use std::{fmt::Debug, sync::atomic};

/// The Node datastructure used for the unbounded Queue
struct Node<T> {
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
                n.next.store(std::ptr::null_mut(), atomic::Ordering::SeqCst);
                n
            }
            Err(_) => Box::new(Node {
                data: Some(data),
                next: atomic::AtomicPtr::new(std::ptr::null_mut()),
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

impl<T> Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedSender ()")
    }
}

/// The Unbounded Receiver Half
pub struct UnboundedReceiver<T> {
    head: *mut Node<T>,
    node_return: bounded::BoundedSender<Box<Node<T>>>,
}

impl<T> UnboundedReceiver<T> {
    /// Attempts to dequeue a piece of Data
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        let prev_head = unsafe { Box::from_raw(self.head) };
        let next_ptr = prev_head.next.load(atomic::Ordering::SeqCst);
        if next_ptr.is_null() {
            std::mem::forget(prev_head);
            return Err(DequeueError::WouldBlock);
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

impl<T> Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedReceiver ()")
    }
}

/// Creates a new Unbounded Queue-Pair
pub fn unbounded_basic_queue<T>() -> (UnboundedReceiver<T>, UnboundedSender<T>) {
    let (node_rx, node_tx) = bounded::bounded_queue(64);
    let dummy_node = Box::new(Node {
        data: None,
        next: atomic::AtomicPtr::new(std::ptr::null_mut()),
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

        assert_eq!(Err(DequeueError::WouldBlock), rx.try_dequeue());
        tx.enqueue(13);
        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Err(DequeueError::WouldBlock), rx.try_dequeue());
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
