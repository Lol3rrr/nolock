use std::sync::atomic;

use super::UnderlyingQueue;

mod entry_data;
use entry_data::QueueEntryData;

/// A single Entry in the Queue
#[derive(Debug)]
struct QueueEntry(atomic::AtomicU64);

impl QueueEntry {
    /// Creates a new QueueEntry
    pub fn new(invalid_index: u32) -> Self {
        let data = QueueEntryData::new(true, 0, invalid_index);
        Self(atomic::AtomicU64::new(data.into()))
    }

    /// Loads the underlying U64 into a valid QueueEntryData
    pub fn load(&self, order: atomic::Ordering) -> QueueEntryData {
        QueueEntryData::from(self.0.load(order))
    }

    /// Turns both `current` and `new` into u64's and then uses them for a
    /// compare_exchange on the underlying Atomic-U64
    pub fn cas<C, N>(
        &self,
        current: C,
        new: N,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<u64, u64>
    where
        C: Into<u64>,
        N: Into<u64>,
    {
        self.0
            .compare_exchange(current.into(), new.into(), success, failure)
    }

    /// Turns the given Value into a u64 and then stores that new Value into
    /// the underlying Atomic-U64
    pub fn store<N>(&self, new: N, order: atomic::Ordering)
    where
        N: Into<u64>,
    {
        self.0.store(new.into(), order)
    }
}

#[derive(Debug)]
pub struct Queue {
    /// The Number of usable Elements in the Queue
    size: usize,
    /// The Index used to mark an Entry as invalid
    invalid_index: u32,
    /// The underlying Buffer for all QueueEntries
    entries: Vec<QueueEntry>,
    /// The Head of the Queue
    head: atomic::AtomicUsize,
    /// The Tail of the Queue
    tail: atomic::AtomicUsize,
    /// The current Threshold
    threshold: atomic::AtomicIsize,
}

impl Queue {
    /// Creates a new empty Queue with the given Capacity/Size
    pub fn new(capacity: usize) -> Self {
        // Calculate the invalid Index to use for this Queue
        let invalid_index = (2 * capacity - 1) as u32;

        // Create the Entries-Buffer
        let entries = {
            let mut tmp = Vec::with_capacity(2 * capacity);
            for _ in 0..(2 * capacity) {
                tmp.push(QueueEntry::new(invalid_index));
            }
            tmp
        };

        Self {
            size: capacity,
            invalid_index,
            entries,
            head: atomic::AtomicUsize::new(capacity * 2),
            tail: atomic::AtomicUsize::new(capacity * 2),
            threshold: atomic::AtomicIsize::new(-1),
        }
    }

    /// Calculates the Cycle for a given Tail/Head index
    fn cycle(raw: usize, capacity: usize) -> u32 {
        (raw / (capacity * 2)) as u32
    }

    fn catchup(&self, mut head: usize, mut tail: usize) {
        loop {
            if self
                .tail
                .compare_exchange(
                    tail,
                    head,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                )
                .is_ok()
            {
                return;
            }

            head = self.head.load(atomic::Ordering::Acquire);
            tail = self.head.load(atomic::Ordering::Acquire);
            if tail >= head {
                return;
            }
        }
    }
}

impl UnderlyingQueue for Queue {
    fn enqueue(&self, index: usize) {
        loop {
            let tail = self.tail.fetch_add(1, atomic::Ordering::AcqRel);
            let tail_cycle = Self::cycle(tail, self.size);
            let j = tail % (self.size * 2);

            let entry = self.entries.get(j).expect("");

            loop {
                let raw_entry = entry.load(atomic::Ordering::Acquire);
                let entry_cycle = raw_entry.cycle();
                let entry_index = raw_entry.index();

                if entry_cycle < tail_cycle
                    && entry_index == self.invalid_index
                    && (raw_entry.is_safe() || self.head.load(atomic::Ordering::Acquire) <= tail)
                {
                    let new_value = QueueEntryData::new(true, tail_cycle, index as u32);
                    if entry
                        .cas(
                            raw_entry,
                            new_value,
                            atomic::Ordering::AcqRel,
                            atomic::Ordering::Relaxed,
                        )
                        .is_err()
                    {
                        continue;
                    }

                    let thres_chk = (self.size * 3 - 1) as isize;
                    if self.threshold.load(atomic::Ordering::Acquire) != thres_chk {
                        self.threshold.store(thres_chk, atomic::Ordering::Release);
                    }

                    return;
                }
                break;
            }
        }
    }

    fn dequeue(&self) -> Option<usize> {
        if self.threshold.load(atomic::Ordering::Acquire) < 0 {
            return None;
        }

        loop {
            let head = self.head.fetch_add(1, atomic::Ordering::AcqRel);
            let head_cycle = Self::cycle(head, self.size);
            let j = head % (self.size * 2);

            let entry = self.entries.get(j).expect("");
            loop {
                let entry_data = entry.load(atomic::Ordering::Acquire);

                let entry_cycle = entry_data.cycle();
                let entry_index = entry_data.index();
                let entry_safe = entry_data.is_safe();

                if entry_cycle == head_cycle {
                    entry.store(
                        QueueEntryData::new(entry_safe, entry_cycle, self.invalid_index),
                        atomic::Ordering::Release,
                    );
                    return Some(entry_index as usize);
                }

                let new = if entry_index == self.invalid_index {
                    QueueEntryData::new(entry_safe, head_cycle, self.invalid_index)
                } else {
                    QueueEntryData::new(false, entry_cycle, entry_index)
                };

                if entry_cycle < head_cycle {
                    if entry
                        .cas(
                            entry_data,
                            new,
                            atomic::Ordering::AcqRel,
                            atomic::Ordering::Relaxed,
                        )
                        .is_err()
                    {
                        continue;
                    }
                }

                let tail = self.tail.load(atomic::Ordering::Acquire);
                if tail <= head + 1 {
                    self.catchup(head, tail);
                    self.threshold.fetch_add(-1, atomic::Ordering::AcqRel);
                    return None;
                }

                if self.threshold.fetch_add(-1, atomic::Ordering::AcqRel) <= 0 {
                    return None;
                }

                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scq_new() {
        Queue::new(10);
    }
    #[test]
    fn scq_enqueue_single() {
        let queue = Queue::new(10);
        queue.enqueue(13);
    }
    #[test]
    fn scq_enqueue_dequeue_single() {
        let queue = Queue::new(10);
        queue.enqueue(13);
        assert_eq!(Some(13), queue.dequeue());
    }
    #[test]
    fn scq_enqueue_dequeue_fill_multiple() {
        let queue = Queue::new(10);

        for index in 0..(3 * 10) {
            queue.enqueue(index);
            assert_eq!(Some(index), queue.dequeue());
        }
    }
}
