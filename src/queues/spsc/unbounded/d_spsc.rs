//! The Basic and slower version

use super::super::bounded;
use crate::queues::DequeueError;

use std::{
    fmt::Debug,
    sync::{atomic, Arc},
};

/// The Node datastructure used for the unbounded Queue
struct Node<T> {
    data: Option<T>,
    previous: *mut Self,
    next: atomic::AtomicPtr<Node<T>>,
}

/// The Unbounded Sender Half
pub struct UnboundedSender<T> {
    /// Indicates whether or not the Queue has been closed
    closed: Arc<atomic::AtomicBool>,
    /// The Tail of the Queue
    tail: *mut Node<T>,
    /// Receiver for empty Nodes that were consumed by the Queue-Receiver and
    /// are now ready to be reused in order to reduce the impact of dynamic
    /// memory allocation
    node_receiver: bounded::BoundedReceiver<Box<Node<T>>>,
}

impl<T> UnboundedSender<T> {
    /// Creates a new Node with the given Data already stored in the Node
    fn create_new_node(&mut self, data: T, previous: *mut Node<T>) -> Box<Node<T>> {
        // Attempt to receive a new "recycled" Node
        match self.node_receiver.try_dequeue() {
            // We received a "recycled" Node that we can use
            Ok(mut n) => {
                // Overwrite the Data
                n.data = Some(data);
                n.previous = previous;
                // Reset the Next-Ptr to null as this will be the new Tail
                n.next
                    .store(std::ptr::null_mut(), atomic::Ordering::Release);
                n
            }
            // There was no Node waiting to be used again
            //
            // We then simply create a new Node with the given Data that has no
            // next Ptr and then allocate it on the Heap, using the Box
            Err(_) => Box::new(Node {
                data: Some(data),
                previous,
                next: atomic::AtomicPtr::new(std::ptr::null_mut()),
            }),
        }
    }

    /// Enqueues the given Data
    pub fn enqueue(&mut self, data: T) -> Result<(), T> {
        if self.closed.load(atomic::Ordering::Acquire) {
            return Err(data);
        }

        // Obtain a new Node with the given Data already set as the Data field
        let node = self.create_new_node(data, self.tail);

        // Get a PTR to the node
        let node_ptr = Box::into_raw(node);
        // Load the current Tail to append the new Node to the end
        let cur_tail = unsafe { &*self.tail };
        // Actually append the new Node to the Tail
        cur_tail.next.store(node_ptr, atomic::Ordering::Release);
        // Stores the new Node as the current tail of the Queue
        self.tail = node_ptr;

        Ok(())
    }
}

impl<T> Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedSender ()")
    }
}

impl<T> Drop for UnboundedSender<T> {
    fn drop(&mut self) {
        match self.closed.compare_exchange(
            false,
            true,
            atomic::Ordering::SeqCst,
            atomic::Ordering::SeqCst,
        ) {
            Ok(_) => {}
            Err(_) => {
                let mut current_ptr = self.tail;
                while !current_ptr.is_null() {
                    let current = unsafe { Box::from_raw(current_ptr) };
                    current_ptr = current.previous;

                    drop(current);
                }
            }
        };
    }
}

/// The Unbounded Receiver Half
pub struct UnboundedReceiver<T> {
    /// Indicates whether or not the Queue has been closed
    closed: Arc<atomic::AtomicBool>,
    /// The current Head of the Queue
    head: *mut Node<T>,
    /// The Queue to return old Nodes to, to help remove the impact of dynamic
    /// memory managment
    node_return: bounded::BoundedSender<Box<Node<T>>>,
}

impl<T> UnboundedReceiver<T> {
    /// Attempts to dequeue a piece of Data
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Loads the current Head of the Queue
        let prev_head = unsafe { &*self.head };
        // Loads the PTR to the next Element in the Queue
        let next_ptr = prev_head.next.load(atomic::Ordering::Acquire);
        // If the PTR is null, than the Queue is empty
        if next_ptr.is_null() {
            // Return the right error to indicate that there is currently
            // nothing to load
            return Err(DequeueError::Empty);
        }

        // Load the next Entry
        let next = unsafe { &mut *next_ptr };
        // Take out the Data from the next Entry
        let data = next.data.take().unwrap();

        let prev_head_ptr = self.head;

        // Replace the current Head with the next Element
        self.head = next_ptr;

        // Attempt to "recycle" the previous Head
        if let Err((node, _)) = self
            .node_return
            .try_enqueue(unsafe { Box::from_raw(prev_head_ptr) })
        {
            // If the previous Head could not be reused, simply drop/free it
            drop(node);
        }

        Ok(data)
    }

    /// Checks if the Queue contains at least one more element to dequeue
    pub fn has_next(&self) -> bool {
        let prev_head = unsafe { &*self.head };
        let next_ptr = prev_head.next.load(atomic::Ordering::Acquire);

        !next_ptr.is_null()
    }
}

impl<T> Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedReceiver ()")
    }
}

impl<T> Drop for UnboundedReceiver<T> {
    fn drop(&mut self) {
        match self.closed.compare_exchange(
            false,
            true,
            atomic::Ordering::SeqCst,
            atomic::Ordering::SeqCst,
        ) {
            Ok(_) => {}
            Err(_) => {
                let mut current_ptr = self.head;
                while !current_ptr.is_null() {
                    let current = unsafe { Box::from_raw(current_ptr) };
                    current_ptr = current.next.load(atomic::Ordering::Acquire);

                    drop(current);
                }
            }
        };
    }
}

/// Creates a new Unbounded Queue-Pair
pub fn unbounded_basic_queue<T>() -> (UnboundedReceiver<T>, UnboundedSender<T>) {
    let (node_rx, node_tx) = bounded::queue(64);
    let dummy_node = Box::new(Node {
        data: None,
        previous: std::ptr::null_mut(),
        next: atomic::AtomicPtr::new(std::ptr::null_mut()),
    });
    let dummy_ptr = Box::into_raw(dummy_node);

    let closed = Arc::new(atomic::AtomicBool::new(false));

    (
        UnboundedReceiver {
            closed: closed.clone(),
            head: dummy_ptr,
            node_return: node_tx,
        },
        UnboundedSender {
            closed,
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

        tx.enqueue(13).unwrap();
        assert_eq!(Ok(13), rx.try_dequeue());
    }
    #[test]
    fn dequeue_empty() {
        let (mut rx, mut tx) = unbounded_basic_queue();

        assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
        tx.enqueue(13).unwrap();
        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Err(DequeueError::Empty), rx.try_dequeue());
    }
    #[test]
    fn multiple_enqueue_dequeue() {
        let (mut rx, mut tx) = unbounded_basic_queue();

        tx.enqueue(13).unwrap();
        assert_eq!(Ok(13), rx.try_dequeue());
        tx.enqueue(14).unwrap();
        assert_eq!(Ok(14), rx.try_dequeue());
        tx.enqueue(15).unwrap();
        assert_eq!(Ok(15), rx.try_dequeue());
    }
    #[test]
    fn multiple_enqueue_dequeue_2() {
        let (mut rx, mut tx) = unbounded_basic_queue();

        tx.enqueue(13).unwrap();
        tx.enqueue(14).unwrap();
        tx.enqueue(15).unwrap();
        assert_eq!(Ok(13), rx.try_dequeue());
        assert_eq!(Ok(14), rx.try_dequeue());
        assert_eq!(Ok(15), rx.try_dequeue());
    }
}
