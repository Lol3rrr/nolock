use alloc::vec::Vec;
use core::sync::atomic;

use super::UnderlyingQueue;

struct QueueEntry(atomic::AtomicU64);

impl QueueEntry {
    /// Creates a new empty Queue-Entry with the Cycle and Index set to 0
    pub fn new() -> Self {
        Self(atomic::AtomicU64::new(0))
    }

    pub fn load(&self, order: atomic::Ordering) -> u64 {
        self.0.load(order)
    }

    pub fn cas(
        &self,
        previous: u64,
        cycle: u32,
        index: u32,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<u64, u64> {
        let value = ((cycle as u64) << 32) | (index as u64);
        self.0.compare_exchange(previous, value, success, failure)
    }

    pub fn cycle(entry: u64) -> u32 {
        (entry >> 32) as u32
    }
    pub fn index(entry: u64) -> u32 {
        entry as u32
    }
}

pub struct Queue {
    entries: Vec<QueueEntry>,
    head: atomic::AtomicUsize,
    tail: atomic::AtomicUsize,
}

impl Queue {
    /// Creates a new Empty Queue
    pub fn new(capacity: usize) -> Self {
        let entries = {
            let mut tmp = Vec::with_capacity(capacity);
            for _ in 0..capacity {
                tmp.push(QueueEntry::new());
            }
            tmp
        };

        Self {
            entries,
            head: atomic::AtomicUsize::new(capacity),
            tail: atomic::AtomicUsize::new(capacity),
        }
    }

    /// Calculates the current Cycle of the given Value
    fn cycle(tail: usize, capacity: usize) -> u32 {
        (tail / capacity) as u32
    }
}

impl UnderlyingQueue for Queue {
    fn enqueue(&self, index: usize) {
        let tail = loop {
            let tail = self.tail.load(atomic::Ordering::Acquire);
            let tail_cycle = Self::cycle(tail, self.entries.capacity());
            let j = tail % self.entries.capacity();

            let entry = self.entries.get(j).expect("Because we always wrap around once we reach the end of the Vector, we can be sure that the Index we try to access is in the Vec itself");

            let raw_entry = entry.load(atomic::Ordering::Acquire);
            let entry_cycle = QueueEntry::cycle(raw_entry);
            if entry_cycle == tail_cycle {
                let _ = self.tail.compare_exchange(
                    tail,
                    tail + 1,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                );
                continue;
            }
            if entry_cycle + 1 != tail_cycle {
                continue;
            }

            if entry
                .cas(
                    raw_entry,
                    tail_cycle,
                    index as u32,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                break tail;
            }
        };

        let _ = self.tail.compare_exchange(
            tail,
            tail + 1,
            atomic::Ordering::AcqRel,
            atomic::Ordering::Relaxed,
        );
    }

    fn dequeue(&self) -> Option<usize> {
        let raw_index = loop {
            let head = self.head.load(atomic::Ordering::Acquire);
            let head_cycle = Self::cycle(head, self.entries.capacity());
            let j = head % self.entries.capacity();

            let entry = self.entries.get(j).expect("");

            let raw_entry = entry.load(atomic::Ordering::Acquire);
            let entry_cycle = QueueEntry::cycle(raw_entry);

            if entry_cycle != head_cycle {
                if entry_cycle + 1 == head_cycle {
                    return None;
                }

                continue;
            }

            if self
                .head
                .compare_exchange(
                    head,
                    head + 1,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                break QueueEntry::index(raw_entry);
            }
        };

        Some(raw_index as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_queue() {
        Queue::new(10);
    }

    #[test]
    fn enqueue_single() {
        let queue = Queue::new(10);

        queue.enqueue(13);
    }
    #[test]
    fn enqueue_dequeue_single() {
        let queue = Queue::new(10);

        queue.enqueue(13);
        assert_eq!(Some(13), queue.dequeue());
    }
    #[test]
    fn enqueue_dequeue_double_capacity() {
        let queue = Queue::new(10);

        for index in 0..20 {
            queue.enqueue(index);
            assert_eq!(Some(index), queue.dequeue());
        }
    }
}
