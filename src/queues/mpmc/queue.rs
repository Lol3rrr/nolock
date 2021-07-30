use std::{cell::UnsafeCell, mem::MaybeUninit};

mod ncq;
mod scq;

pub struct Bounded<T, UQ> {
    data: Vec<UnsafeCell<MaybeUninit<T>>>,
    aq: UQ,
    fq: UQ,
}

pub trait UnderlyingQueue {
    /// Enqueues the given Index
    fn enqueue(&self, index: usize);
    /// Attempts to dequeue some Index
    fn dequeue(&self) -> Option<usize>;
}

impl<T, UQ> Bounded<T, UQ> {
    pub fn new(aq: UQ, fq: UQ, capacity: usize) -> Self {
        let data = {
            let mut tmp = Vec::with_capacity(capacity);
            for _ in 0..capacity {
                tmp.push(UnsafeCell::new(MaybeUninit::uninit()));
            }
            tmp
        };

        Self { data, aq, fq }
    }
}

impl<T> Bounded<T, ncq::Queue> {
    pub fn new_ncq(capacity: usize) -> Self {
        let aq = ncq::Queue::new(capacity);
        let fq = ncq::Queue::new(capacity);

        for index in 0..capacity {
            fq.enqueue(index);
        }

        Self::new(aq, fq, capacity)
    }
}
impl<T> Bounded<T, scq::Queue> {
    pub fn new_scq(capacity: usize) -> Self {
        let aq = scq::Queue::new(capacity);
        let fq = scq::Queue::new(capacity);

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
    pub fn enqueue(&self, data: T) -> Result<(), T> {
        let index = match self.fq.dequeue() {
            Some(i) => i,
            None => return Err(data),
        };

        let bucket = self
            .data
            .get(index)
            .expect("The received Index should always be in the Bounds of the Data Buffer");

        // TODO
        // Write a proper safety comment as to why this is always allowed
        let bucket_ptr = bucket.get();
        unsafe { bucket_ptr.write(MaybeUninit::new(data)) };

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

        // TODO
        // Write a proper safety comment
        let bucket_ptr = bucket.get();
        let data = unsafe { bucket_ptr.replace(MaybeUninit::uninit()).assume_init() };

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
    fn ncq_enqueue_dequeue() {
        let queue = Bounded::<u64, ncq::Queue>::new_ncq(10);

        assert_eq!(Ok(()), queue.enqueue(15));
        assert_eq!(Some(15), queue.dequeue());
    }
    #[test]
    fn scq_enqueue_dequeue() {
        let queue = Bounded::<u64, scq::Queue>::new_scq(10);

        assert_eq!(Ok(()), queue.enqueue(15));
        assert_eq!(Some(15), queue.dequeue());
    }
}
