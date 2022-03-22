use crate::sync::atomic;
use std::{cell::UnsafeCell, mem::MaybeUninit, sync::Arc};

use crate::queues::{DequeueError, EnqueueError};

mod scq;

pub struct BoundedQueue<T> {
    /// The actual Buffer for all the Data-Entries
    data: Arc<Vec<UnsafeCell<MaybeUninit<T>>>>,
    /// The "available"-Queue, contains all the Indices at which Data is currently
    /// stored and can be read from
    pub aq: Arc<scq::Queue>,
    /// The Queue for all the free Indices at which no Data is stored and
    /// therefore can be used to store Data in
    fq: Arc<scq::Queue>,

    pub next: atomic::AtomicPtr<Self>,
}

pub fn new_queue<T>(capacity: usize) -> BoundedQueue<T> {
    let data = {
        // Creates a Vec with the given Capacity
        let mut tmp = Vec::with_capacity(capacity);
        // Add empty Data-Points to the Vec, until its capacity is reached
        for _ in 0..capacity {
            tmp.push(UnsafeCell::new(MaybeUninit::uninit()));
        }
        Arc::new(tmp)
    };

    // Create both of the needed Queues
    let aq = scq::Queue::new(capacity);
    let fq = scq::Queue::new(capacity);

    // Fill `fq` with all the available Indices, in this case 0-capacity
    for index in 0..capacity {
        fq.enqueue(index).expect("Works as expected");
    }

    let aq_arc = Arc::new(aq);
    let fq_arc = Arc::new(fq);

    BoundedQueue {
        data,
        aq: aq_arc,
        fq: fq_arc,
        next: atomic::AtomicPtr::new(std::ptr::null_mut()),
    }
}

// Safety:
// This is safe to do because the Queue itself is sync and upholds the variant
// for giving "synchronized" access to the underlying Data by the nature of the
// algorithm.
// Whether or not T is Sync is actually not important because we never actually
// use T anywhere in the Code but instead just pass it around
unsafe impl<T> Sync for BoundedQueue<T> {}

// Safety:
// The Queue is only Send if T is send, because even though we dont use T in
// the Algorithm, we still store it. Therefore if you can't send T across
// threads you can't send the Queue across Threads, because we also store them
// and would therefore try to send them across Threads.
unsafe impl<T> Send for BoundedQueue<T> where T: Send {}

impl<T> BoundedQueue<T> {
    /// Attempts to enqueue an item on the Queue
    ///
    /// # Returns
    /// * `Ok(())` if the item was successfully enqueued
    /// * `Err(data)` if the Queue is full and the item could not be enqueued
    pub fn try_enqueue(&self, data: T) -> Result<(), (EnqueueError, T)> {
        // Attempt to get a free-Index to insert the data into
        let index = match self.fq.dequeue() {
            Some(i) => i,
            None => {
                self.aq.finalize();

                return Err((EnqueueError::Full, data));
            }
        };

        // Actually obtain the Bucket to insert into
        let bucket = self
            .data
            .get(index)
            .expect("The received Index should always be in the Bounds of the Data Buffer");

        // # Safety:
        // It is safe to get mutable access to the single Bucket of Data, because we got the index
        // of the Bucket from the Queue of free indices.
        //
        // Every index only exists once in either the free-Qeueu or the available-Queue and therefore
        // no two or more threads can obtain the same index at the same time and attempt to write to it
        // or read from it.
        let bucket_ptr = bucket.get();
        unsafe { bucket_ptr.write(MaybeUninit::new(data)) };

        // Enqueue the now filled index into the Queue for Indices that contain data
        match self.aq.enqueue(index) {
            Ok(_) => Ok(()),
            Err(_) => {
                let bucket_ptr = bucket.get();
                let old = unsafe { bucket_ptr.replace(MaybeUninit::uninit()).assume_init() };

                Err((EnqueueError::Full, old))
            }
        }
    }

    pub fn dequeue(&self) -> Result<T, DequeueError> {
        let index = match self.aq.dequeue() {
            Some(i) => i,
            None => return Err(DequeueError::Empty),
        };

        let bucket = self
            .data
            .get(index)
            .expect("The received Index should always be in the Bounds of the Data-Buffer");

        // # Safety:
        // It is safe to get mutable access to the single Bucket of Data, because we got the index
        // of the Bucket from the Queue of free indices.
        //
        // Every index only exists once in either the free-Qeueu or the available-Queue and therefore
        // no two or more threads can obtain the same index at the same time and attempt to write to it
        // or read from it.
        let bucket_ptr = bucket.get();
        let data = unsafe { bucket_ptr.replace(MaybeUninit::uninit()).assume_init() };

        self.fq.enqueue(index).expect("");

        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_finalize() {
        let queue = new_queue(10);

        for index in 0..10 {
            queue
                .try_enqueue(index)
                .expect("Queue has enough capacity to place all the Elements into it");
        }

        // This will finalize it
        assert_eq!(Err((EnqueueError::Full, 0)), queue.try_enqueue(0));

        queue.dequeue().expect("The Queue contains elements");

        // The Queue has been finalized and therefore does not accept any new Entries even
        // if there is now an empty slot in it
        assert_eq!(Err((EnqueueError::Full, 0)), queue.try_enqueue(0));
    }
}
