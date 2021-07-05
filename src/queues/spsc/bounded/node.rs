use std::{cell::UnsafeCell, sync::atomic};

/// A Node is a single Entry in the Buffer of the Queue
pub struct Node<T> {
    /// The actual Data stored in the Node itself
    data: UnsafeCell<Option<T>>,
    /// Indicates whether or not the current Node actually contains Data
    is_set: atomic::AtomicBool,
}

impl<T> Node<T> {
    /// Creates a new Empty Node
    pub fn new() -> Self {
        Self {
            data: UnsafeCell::new(None),
            is_set: atomic::AtomicBool::new(false),
        }
    }

    /// Checks if the current Node is marked as `set` and actually contains
    /// Data that could be read
    pub fn is_set(&self) -> bool {
        self.is_set.load(atomic::Ordering::Acquire)
    }

    /// Stores the given Data into the current Node and marks the Node as being
    /// `set` and ready to be consumed
    pub fn store(&self, data: T) {
        // Get the mutable access to the underlying Data in order to overwrite
        // it with the given new Data
        let d_ptr = self.data.get();
        let mut_data = unsafe { &mut *d_ptr };

        // Actually store the Data into the Node
        mut_data.replace(data);

        // Mark the Node as `set` again
        self.is_set.store(true, atomic::Ordering::Release);
    }

    /// Attempts to load the current Data from the Node and marks the Data as
    /// empty again
    pub fn load(&self) -> T {
        // Get the mutable access to the underlying Data in order to properly
        // take it out and replace it with empty Data
        let d_ptr = self.data.get();
        let mut_data = unsafe { &mut *d_ptr };

        // Take the Data out of the Option
        let data = mut_data.take().unwrap();
        // Mark the Node as empty again
        self.is_set.store(false, atomic::Ordering::Release);

        // Return the Data
        data
    }
}
