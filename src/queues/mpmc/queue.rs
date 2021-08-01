use std::{cell::UnsafeCell, mem::MaybeUninit};

pub mod ncq;
pub mod scq;

/// A generic Version of the described Queue, which allows for different
/// implementations of the Queue for `aq` and `fq`.
pub struct Bounded<T, UQ> {
    /// The actual Buffer for all the Data-Entries
    data: Vec<UnsafeCell<MaybeUninit<T>>>,
    /// The "available"-Queue, contains all the Indices at which Data is currently
    /// stored and can be read from
    aq: UQ,
    /// The Queue for all the free Indices at which no Data is stored and
    /// therefore can be used to store Data in
    fq: UQ,
}

/// This trait needs to be implemented by the Underlying-Queue that is used for
/// the `aq` and `fq` Queues in the overall Queue.
pub trait UnderlyingQueue {
    /// Enqueues the given Index
    fn enqueue(&self, index: usize);
    /// Attempts to dequeue some Index
    fn dequeue(&self) -> Option<usize>;
}

impl<T, UQ> Bounded<T, UQ> {
    /// Creates a new Queue with the given Capacity and underlying Queues
    fn new(aq: UQ, fq: UQ, capacity: usize) -> Self {
        let data = {
            // Creates a Vec with the given Capacity
            let mut tmp = Vec::with_capacity(capacity);
            // Add empty Data-Points to the Vec, until its capacity is reached
            for _ in 0..capacity {
                tmp.push(UnsafeCell::new(MaybeUninit::uninit()));
            }
            tmp
        };

        Self { data, aq, fq }
    }
}

// Safety:
// TODO
unsafe impl<T, UQ> Sync for Bounded<T, UQ> {}

impl<T> Bounded<T, ncq::Queue> {
    /// Creates a new Queue with the given `capacity` using [`ncq`] for the underlying Queues
    pub fn new_ncq(capacity: usize) -> Self {
        // Create both of the needed Queues
        let aq = ncq::Queue::new(capacity);
        let fq = ncq::Queue::new(capacity);

        // Fill `fq` with all the available Indices, in this case 0-capacity
        for index in 0..capacity {
            fq.enqueue(index);
        }

        Self::new(aq, fq, capacity)
    }
}
impl<T> Bounded<T, scq::Queue> {
    /// Creates a new Queue with the given `capacity` using [`scq`] for the underlying Queues
    pub fn new_scq(capacity: usize) -> Self {
        // Create both of the needed Queues
        let aq = scq::Queue::new(capacity);
        let fq = scq::Queue::new(capacity);

        // Fill `fq` with all the available Indices, in this case 0-capacity
        for index in 0..capacity {
            fq.enqueue(index);
        }

        Self::new(aq, fq, capacity)
    }
}

impl<T, UQ> Bounded<T, UQ>
where
    UQ: UnderlyingQueue,
{
    /// Attempts to enqueue an item on the Queue
    ///
    /// # Returns
    /// * `Ok(())` if the item was successfully enqueued
    /// * `Err(data)` if the Queue is full and the item could not be enqueued
    pub fn try_enqueue(&self, data: T) -> Result<(), T> {
        // Attempt to get a free-Index to insert the data into
        let index = match self.fq.dequeue() {
            Some(i) => i,
            None => return Err(data),
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
        self.aq.enqueue(index);
        Ok(())
    }

    pub fn dequeue(&self) -> Option<T> {
        let index = match self.aq.dequeue() {
            Some(i) => i,
            None => return None,
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

        self.fq.enqueue(index);

        Some(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncq_new() {
        Bounded::<u64, ncq::Queue>::new_ncq(10);
    }
    #[test]
    fn ncq_enqueue() {
        let queue = Bounded::<u64, ncq::Queue>::new_ncq(10);

        assert_eq!(Ok(()), queue.try_enqueue(15));
    }
    #[test]
    fn ncq_dequeue() {
        let queue = Bounded::<u64, ncq::Queue>::new_ncq(10);

        assert_eq!(None, queue.dequeue());
    }
    #[test]
    fn ncq_enqueue_dequeue() {
        let queue = Bounded::<u64, ncq::Queue>::new_ncq(10);

        assert_eq!(Ok(()), queue.try_enqueue(15));
        assert_eq!(Some(15), queue.dequeue());
    }
    #[test]
    fn ncq_enqueue_dequeue_fill_multiple() {
        let queue = Bounded::<usize, ncq::Queue>::new_ncq(10);

        for index in 0..(5 * 10) {
            assert_eq!(Ok(()), queue.try_enqueue(index));
            assert_eq!(Some(index), queue.dequeue());
        }
    }

    #[test]
    fn scq_new() {
        Bounded::<u64, scq::Queue>::new_scq(10);
    }
    #[test]
    fn scq_enqueue() {
        let queue = Bounded::<u64, scq::Queue>::new_scq(10);

        assert_eq!(Ok(()), queue.try_enqueue(15));
    }
    #[test]
    fn scq_dequeue() {
        let queue = Bounded::<u64, scq::Queue>::new_scq(10);

        assert_eq!(None, queue.dequeue());
    }
    #[test]
    fn scq_enqueue_dequeue() {
        let queue = Bounded::<u64, scq::Queue>::new_scq(10);

        assert_eq!(Ok(()), queue.try_enqueue(15));
        assert_eq!(Some(15), queue.dequeue());
    }
    #[test]
    fn scq_enqueue_dequeue_fill_multiple() {
        let queue = Bounded::<usize, scq::Queue>::new_scq(10);

        for index in 0..(5 * 10) {
            assert_eq!(Ok(()), queue.try_enqueue(index));
            assert_eq!(Some(index), queue.dequeue());
        }
    }
}
