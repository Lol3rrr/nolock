//! The Basic and slower version

use super::super::{bounded, DequeueError};

use std::{fmt::Debug, mem::ManuallyDrop, sync::atomic};

/// The Node datastructure used for the unbounded Queue
struct Node<T> {
    data: Option<T>,
    next: atomic::AtomicPtr<Node<T>>,
}

/// The Unbounded Sender Half
pub struct UnboundedSender<T> {
    /// The Tail of the Queue
    tail: *mut Node<T>,
    /// Receiver for empty Nodes that were consumed by the Queue-Receiver and
    /// are now ready to be reused in order to reduce the impact of dynamic
    /// memory allocation
    node_receiver: bounded::BoundedReceiver<Box<Node<T>>>,
}

impl<T> UnboundedSender<T> {
    /// Creates a new Node with the given Data already stored in the Node
    fn create_new_node(&mut self, data: T) -> Box<Node<T>> {
        // Attempt to receive a new "recycled" Node
        match self.node_receiver.try_dequeue() {
            // We received a "recycled" Node that we can use
            Ok(mut n) => {
                // Overwrite the Data
                n.data = Some(data);
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
                next: atomic::AtomicPtr::new(std::ptr::null_mut()),
            }),
        }
    }

    /// Loads the current Tail of the Queue
    fn load_tail(&self) -> ManuallyDrop<Box<Node<T>>> {
        let boxed = unsafe { Box::from_raw(self.tail) };
        ManuallyDrop::new(boxed)
    }

    /// Enqueues the given Data
    pub fn enqueue(&mut self, data: T) {
        // Obtain a new Node with the given Data already set as the Data field
        let node = self.create_new_node(data);

        // Get a PTR to the node
        let node_ptr = Box::into_raw(node);
        // Load the current Tail to append the new Node to the end
        let cur_tail = self.load_tail();
        // Actually append the new Node to the Tail
        cur_tail.next.store(node_ptr, atomic::Ordering::Release);
        // Stores the new Node as the current tail of the Queue
        self.tail = node_ptr;
    }
}

impl<T> Debug for UnboundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedSender ()")
    }
}

/// The Unbounded Receiver Half
pub struct UnboundedReceiver<T> {
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
        let prev_head = unsafe { Box::from_raw(self.head) };
        // Loads the PTR to the next Element in the Queue
        let next_ptr = prev_head.next.load(atomic::Ordering::Acquire);
        // If the PTR is null, than the Queue is empty
        if next_ptr.is_null() {
            // Forget the current Head, as we dont want to free it
            std::mem::forget(prev_head);
            // Return the right error to indicate that there is currently
            // nothing to load
            return Err(DequeueError::WouldBlock);
        }

        // Load the next Entry
        let mut next = ManuallyDrop::new(unsafe { Box::from_raw(next_ptr) });
        // Take out the Data from the next Entry
        let data = next.data.take().unwrap();

        // Replace the current Head with the next Element
        self.head = next_ptr;

        // Attempt to "recycle" the previous Head
        if let Err((node, _)) = self.node_return.try_enqueue(prev_head) {
            // If the previous Head could not be reused, simply drop/free it
            drop(node);
        }

        Ok(data)
    }
}

impl<T> Debug for UnboundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UnboundedReceiver ()")
    }
}

/// Creates a new Unbounded Queue-Pair
pub fn unbounded_basic_queue<T>() -> (UnboundedReceiver<T>, UnboundedSender<T>) {
    let (node_rx, node_tx) = bounded::queue(64);
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
