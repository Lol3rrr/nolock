use std::{fmt::Display, sync::atomic};

use super::UnderlyingQueue;

/// Internal Storage
///
/// Bits:
/// 1. IsSafe
/// 2-32. Cycle
/// 33-64. Index
#[derive(Debug)]
struct QueueEntry(atomic::AtomicU64);

struct QueueEntryData(u64);

impl QueueEntryData {
    pub fn new(is_safe: bool, cycle: u32, index: u32) -> Self {
        let base_val = if is_safe { 0x8000000000000000 } else { 0 };
        Self(base_val | (((cycle & 0x7fffffff) as u64) << 32) | (index as u64))
    }
    pub fn to_u64(&self) -> u64 {
        self.0
    }

    pub fn is_safe(&self) -> bool {
        (self.0 >> 63) == 1
    }
    pub fn cycle(&self) -> u32 {
        ((self.0 >> 32) & 0x7fffffff) as u32
    }
    pub fn index(&self) -> u32 {
        (self.0 & 0xffffffff) as u32
    }
}
impl From<u64> for QueueEntryData {
    fn from(data: u64) -> Self {
        Self(data)
    }
}
impl Display for QueueEntryData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "(IsSafe: {}, Cycle: {}, Index: {})",
            self.is_safe(),
            self.cycle(),
            self.index()
        )
    }
}

impl QueueEntry {
    pub fn new(invalid_index: u32) -> Self {
        let data = QueueEntryData::new(true, 0, invalid_index);
        Self(atomic::AtomicU64::new(data.to_u64()))
    }

    pub fn load(&self, order: atomic::Ordering) -> QueueEntryData {
        QueueEntryData::from(self.0.load(order))
    }
    pub fn cas(
        &self,
        current: u64,
        new: u64,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> Result<u64, u64> {
        self.0.compare_exchange(current, new, success, failure)
    }

    pub fn or(&self, other: u64) {
        self.0.fetch_or(other, atomic::Ordering::SeqCst);
    }
    pub fn store(&self, new: u64, order: atomic::Ordering) {
        self.0.store(new, order)
    }
}

#[derive(Debug)]
pub struct Queue {
    size: usize,
    invalid_index: u32,
    entries: Vec<QueueEntry>,
    head: atomic::AtomicUsize,
    tail: atomic::AtomicUsize,
    threshold: atomic::AtomicIsize,
}

impl Queue {
    pub fn new(capacity: usize) -> Self {
        let invalid_index = (2 * capacity - 1) as u32;

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
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                return;
            }

            head = self.head.load(atomic::Ordering::SeqCst);
            tail = self.head.load(atomic::Ordering::SeqCst);
            if tail >= head {
                return;
            }
        }
    }
}

impl UnderlyingQueue for Queue {
    fn enqueue(&self, index: usize) {
        loop {
            let tail = self.tail.fetch_add(1, atomic::Ordering::SeqCst);
            let tail_cycle = Self::cycle(tail, self.size);
            let j = tail % (self.size * 2);

            let entry = self.entries.get(j).expect("");

            loop {
                let raw_entry = entry.load(atomic::Ordering::SeqCst);
                let entry_cycle = raw_entry.cycle();
                let entry_index = raw_entry.index();

                if entry_cycle < tail_cycle
                    && entry_index == self.invalid_index
                    && (raw_entry.is_safe() || self.head.load(atomic::Ordering::SeqCst) <= tail)
                {
                    let new_value = QueueEntryData::new(true, tail_cycle, index as u32);
                    if entry
                        .cas(
                            raw_entry.to_u64(),
                            new_value.to_u64(),
                            atomic::Ordering::SeqCst,
                            atomic::Ordering::SeqCst,
                        )
                        .is_err()
                    {
                        continue;
                    }
                    let thres_chk = (self.size * 3 - 1) as isize;
                    if self.threshold.load(atomic::Ordering::SeqCst) != thres_chk {
                        self.threshold.store(thres_chk, atomic::Ordering::SeqCst);
                    }

                    return;
                }
                break;
            }
        }
    }
    fn dequeue(&self) -> Option<usize> {
        if self.threshold.load(atomic::Ordering::SeqCst) < 0 {
            return None;
        }

        loop {
            let head = self.head.fetch_add(1, atomic::Ordering::SeqCst);
            let head_cycle = Self::cycle(head, self.size);
            let j = head % (self.size * 2);

            let entry = self.entries.get(j).expect("");
            loop {
                let entry_data = entry.load(atomic::Ordering::SeqCst);

                if entry_data.cycle() == head_cycle {
                    entry.store(
                        QueueEntryData::new(
                            entry_data.is_safe(),
                            entry_data.cycle(),
                            self.invalid_index,
                        )
                        .to_u64(),
                        atomic::Ordering::SeqCst,
                    );
                    return Some(entry_data.index() as usize);
                }

                let new = if entry_data.index() == self.invalid_index {
                    QueueEntryData::new(entry_data.is_safe(), head_cycle, self.invalid_index)
                } else {
                    QueueEntryData::new(false, entry_data.cycle(), entry_data.index())
                };

                if entry_data.cycle() < head_cycle {
                    if entry
                        .cas(
                            entry_data.to_u64(),
                            new.to_u64(),
                            atomic::Ordering::SeqCst,
                            atomic::Ordering::SeqCst,
                        )
                        .is_err()
                    {
                        continue;
                    }
                }

                let tail = self.tail.load(atomic::Ordering::SeqCst);
                if tail <= head + 1 {
                    self.catchup(head, tail);
                    self.threshold.fetch_add(-1, atomic::Ordering::SeqCst);
                    return None;
                }

                if self.threshold.fetch_sub(-1, atomic::Ordering::SeqCst) <= 0 {
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
    fn queue_entry_is_safe() {
        assert_eq!(true, QueueEntryData::from(0x8000000000000000).is_safe());
        assert_eq!(true, QueueEntryData::from(0xd000000000000000).is_safe());
        assert_eq!(false, QueueEntryData::from(0x4000000000000000).is_safe());
    }
    #[test]
    fn queue_entry_cycle() {
        assert_eq!(0, QueueEntryData::from(0x8000000000000000).cycle());
        assert_eq!(1, QueueEntryData::from(0x8000000100000000).cycle());
        assert_eq!(0x70000000, QueueEntryData::from(0xf000000000000000).cycle());
    }
    #[test]
    fn queue_entry_index() {
        assert_eq!(0x80000000, QueueEntryData::from(0x8123456780000000).index());
    }
    #[test]
    fn queue_entry_to_value() {
        assert_eq!(0x8000000000000000, QueueEntryData::new(true, 0, 0).to_u64());
        assert_eq!(
            0x8000001500000000,
            QueueEntryData::new(true, 0x15, 0).to_u64()
        );
        assert_eq!(
            0x8000001500000015,
            QueueEntryData::new(true, 0x15, 0x15).to_u64()
        );
    }

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
