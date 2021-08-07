use std::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::{atomic, Arc},
};

use crate::queues::{DequeueError, EnqueueError};

pub mod ncq;
pub mod scq;

/// The Receiver Side of a generic MPMC-Queue, according to the related Paper, which allows for
/// different implementations of the Underlying Queue for `aq` and `fq`
pub struct BoundedReceiver<T, UQ> {
    /// The actual Buffer for all the Data-Entries
    data: Arc<Vec<UnsafeCell<MaybeUninit<T>>>>,
    /// The "available"-Queue, contains all the Indices at which Data is currently
    /// stored and can be read from
    aq: Arc<UQ>,
    /// The Queue for all the free Indices at which no Data is stored and
    /// therefore can be used to store Data in
    fq: Arc<UQ>,
    /// The Number of current Receivers
    rx_count: Arc<atomic::AtomicU64>,
    /// The Number of current Producers
    tx_count: Arc<atomic::AtomicU64>,
}

/// The Sender Side of a generic MPMC-Queue, according to the related Paper, which allows for
/// different implementations of the Underlying Queue for `aq` and `fq`
pub struct BoundedSender<T, UQ> {
    /// The actual Buffer for all the Data-Entries
    data: Arc<Vec<UnsafeCell<MaybeUninit<T>>>>,
    /// The "available"-Queue, contains all the Indices at which Data is currently
    /// stored and can be read from
    aq: Arc<UQ>,
    /// The Queue for all the free Indices at which no Data is stored and
    /// therefore can be used to store Data in
    fq: Arc<UQ>,
    /// The Number of current Receivers
    rx_count: Arc<atomic::AtomicU64>,
    /// The Number of current Producers
    tx_count: Arc<atomic::AtomicU64>,
}

/// This trait needs to be implemented by the Underlying-Queue that is used for
/// the `aq` and `fq` Queues in the overall Queue.
pub trait UnderlyingQueue {
    /// Enqueues the given Index
    fn enqueue(&self, index: usize);
    /// Attempts to dequeue some Index
    fn dequeue(&self) -> Option<usize>;
}

fn new_queue<T, UQ>(
    aq: UQ,
    fq: UQ,
    capacity: usize,
) -> (BoundedReceiver<T, UQ>, BoundedSender<T, UQ>) {
    let data = {
        // Creates a Vec with the given Capacity
        let mut tmp = Vec::with_capacity(capacity);
        // Add empty Data-Points to the Vec, until its capacity is reached
        for _ in 0..capacity {
            tmp.push(UnsafeCell::new(MaybeUninit::uninit()));
        }
        Arc::new(tmp)
    };

    let aq_arc = Arc::new(aq);
    let fq_arc = Arc::new(fq);

    let rx_count = Arc::new(atomic::AtomicU64::new(1));
    let tx_count = Arc::new(atomic::AtomicU64::new(1));

    let rx = BoundedReceiver {
        data: data.clone(),
        aq: aq_arc.clone(),
        fq: fq_arc.clone(),
        rx_count: rx_count.clone(),
        tx_count: tx_count.clone(),
    };
    let tx = BoundedSender {
        data,
        aq: aq_arc,
        fq: fq_arc,
        rx_count,
        tx_count,
    };

    (rx, tx)
}

// Safety:
// TODO
unsafe impl<T, UQ> Sync for BoundedReceiver<T, UQ> where T: Sync {}
unsafe impl<T, UQ> Sync for BoundedSender<T, UQ> where T: Sync {}
// Safety:
// TODO
unsafe impl<T, UQ> Send for BoundedReceiver<T, UQ> where T: Send {}
unsafe impl<T, UQ> Send for BoundedSender<T, UQ> where T: Send {}

pub fn queue_ncq<T>(
    capacity: usize,
) -> (BoundedReceiver<T, ncq::Queue>, BoundedSender<T, ncq::Queue>) {
    // Create both of the needed Queues
    let aq = ncq::Queue::new(capacity);
    let fq = ncq::Queue::new(capacity);

    // Fill `fq` with all the available Indices, in this case 0-capacity
    for index in 0..capacity {
        fq.enqueue(index);
    }

    new_queue(aq, fq, capacity)
}

pub fn queue_scq<T>(
    capacity: usize,
) -> (BoundedReceiver<T, scq::Queue>, BoundedSender<T, scq::Queue>) {
    // Create both of the needed Queues
    let aq = scq::Queue::new(capacity);
    let fq = scq::Queue::new(capacity);

    // Fill `fq` with all the available Indices, in this case 0-capacity
    for index in 0..capacity {
        fq.enqueue(index);
    }

    new_queue(aq, fq, capacity)
}

impl<T, UQ> BoundedSender<T, UQ>
where
    UQ: UnderlyingQueue,
{
    /// Attempts to enqueue an item on the Queue
    ///
    /// # Returns
    /// * `Ok(())` if the item was successfully enqueued
    /// * `Err(data)` if the Queue is full and the item could not be enqueued
    pub fn try_enqueue(&self, data: T) -> Result<(), (EnqueueError, T)> {
        if self.is_closed() {
            return Err((EnqueueError::Closed, data));
        }

        // Attempt to get a free-Index to insert the data into
        let index = match self.fq.dequeue() {
            Some(i) => i,
            None => return Err((EnqueueError::Full, data)),
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

    /// Checks if the Receiving Half of the Queue has been closed
    pub fn is_closed(&self) -> bool {
        self.rx_count.load(atomic::Ordering::Acquire) == 0
    }
}

impl<T, UQ> Drop for BoundedSender<T, UQ> {
    fn drop(&mut self) {
        self.tx_count.fetch_sub(1, atomic::Ordering::AcqRel);
    }
}

impl<T, UQ> BoundedReceiver<T, UQ>
where
    UQ: UnderlyingQueue,
{
    pub fn dequeue(&self) -> Result<T, DequeueError> {
        let index = match self.aq.dequeue() {
            Some(i) => i,
            None => {
                if self.is_closed() {
                    return Err(DequeueError::Closed);
                }

                return Err(DequeueError::Empty);
            }
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

        Ok(data)
    }

    /// Checks if the Sending Half of the Queue has been closed
    pub fn is_closed(&self) -> bool {
        self.tx_count.load(atomic::Ordering::Acquire) == 0
    }
}

impl<T, UQ> Drop for BoundedReceiver<T, UQ> {
    fn drop(&mut self) {
        self.rx_count.fetch_sub(1, atomic::Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ncq_new() {
        queue_ncq::<u64>(10);
    }
    #[test]
    fn scq_new() {
        queue_scq::<u64>(10);
    }

    #[test]
    fn enqueue() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(Ok(()), tx.try_enqueue(15));
        drop(rx);
    }
    #[test]
    fn enqueue_full() {
        let (rx, tx) = queue_ncq::<u64>(10);

        for index in 0..10 {
            assert_eq!(Ok(()), tx.try_enqueue(index));
        }

        assert_eq!(Err((EnqueueError::Full, 15)), tx.try_enqueue(15));
        drop(rx);
    }
    #[test]
    fn enqueue_closed() {
        let (rx, tx) = queue_ncq::<u64>(10);

        drop(rx);
        assert_eq!(Err((EnqueueError::Closed, 15)), tx.try_enqueue(15));
    }
    #[test]
    fn dequeue_empty() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(Err(DequeueError::Empty), rx.dequeue());
        drop(tx);
    }
    #[test]
    fn dequeue_closed() {
        let (rx, tx) = queue_ncq::<u64>(10);

        drop(tx);
        assert_eq!(Err(DequeueError::Closed), rx.dequeue());
    }
    #[test]
    fn enqueue_dequeue() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(Ok(()), tx.try_enqueue(15));
        assert_eq!(Ok(15), rx.dequeue());
    }
    #[test]
    fn enqueue_dequeue_fill_multiple() {
        let (rx, tx) = queue_ncq::<u64>(10);

        for index in 0..(5 * 10) {
            assert_eq!(Ok(()), tx.try_enqueue(index));
            assert_eq!(Ok(index), rx.dequeue());
        }
    }

    #[test]
    fn receiver_closed() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(false, rx.is_closed());

        drop(tx);
        assert_eq!(true, rx.is_closed());
    }
    #[test]
    fn sending_closed() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(false, tx.is_closed());

        drop(rx);
        assert_eq!(true, tx.is_closed());
    }
}
