use std::{cell::UnsafeCell, mem::MaybeUninit, sync::Arc};

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
}
/// The Sender Side of a generic MPMC-Queue, according to the related Paper, which allows for
/// different implementations of the Underlying Queue for `aq` and `fq`
pub struct BoundedSender<T, UQ> {
    data: Arc<Vec<UnsafeCell<MaybeUninit<T>>>>,
    aq: Arc<UQ>,
    fq: Arc<UQ>,
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

    let rx = BoundedReceiver {
        data: data.clone(),
        aq: aq_arc.clone(),
        fq: fq_arc.clone(),
    };
    let tx = BoundedSender {
        data: data.clone(),
        aq: aq_arc,
        fq: fq_arc,
    };
    (rx, tx)
}

// Safety:
// TODO
unsafe impl<T, UQ> Sync for BoundedReceiver<T, UQ> {}
unsafe impl<T, UQ> Sync for BoundedSender<T, UQ> {}
// Safety:
// TODO
unsafe impl<T, UQ> Send for BoundedReceiver<T, UQ> {}
unsafe impl<T, UQ> Send for BoundedSender<T, UQ> {}

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
}

impl<T, UQ> BoundedReceiver<T, UQ>
where
    UQ: UnderlyingQueue,
{
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
        queue_ncq::<u64>(10);
    }
    #[test]
    fn ncq_enqueue() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(Ok(()), tx.try_enqueue(15));
    }
    #[test]
    fn ncq_dequeue() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(None, rx.dequeue());
    }
    #[test]
    fn ncq_enqueue_dequeue() {
        let (rx, tx) = queue_ncq::<u64>(10);

        assert_eq!(Ok(()), tx.try_enqueue(15));
        assert_eq!(Some(15), rx.dequeue());
    }
    #[test]
    fn ncq_enqueue_dequeue_fill_multiple() {
        let (rx, tx) = queue_ncq::<u64>(10);

        for index in 0..(5 * 10) {
            assert_eq!(Ok(()), tx.try_enqueue(index));
            assert_eq!(Some(index), rx.dequeue());
        }
    }

    #[test]
    fn scq_new() {
        queue_scq::<u64>(10);
    }
    #[test]
    fn scq_enqueue() {
        let (rx, tx) = queue_scq::<u64>(10);

        assert_eq!(Ok(()), tx.try_enqueue(15));
    }
    #[test]
    fn scq_dequeue() {
        let (rx, tx) = queue_scq::<u64>(10);

        assert_eq!(None, rx.dequeue());
    }
    #[test]
    fn scq_enqueue_dequeue() {
        let (rx, tx) = queue_scq::<u64>(10);

        assert_eq!(Ok(()), tx.try_enqueue(15));
        assert_eq!(Some(15), rx.dequeue());
    }
    #[test]
    fn scq_enqueue_dequeue_fill_multiple() {
        let (rx, tx) = queue_scq::<u64>(10);

        for index in 0..(5 * 10) {
            assert_eq!(Ok(()), tx.try_enqueue(index));
            assert_eq!(Some(index), rx.dequeue());
        }
    }
}
