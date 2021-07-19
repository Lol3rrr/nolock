//! TODO

use std::sync::{atomic, Arc};

/// TODO
pub struct Queue {
    order: usize,
    tail: atomic::AtomicUsize,
    head: atomic::AtomicUsize,
    entries: Arc<Vec<atomic::AtomicU64>>,

    threshold: atomic::AtomicIsize,
}

impl Queue {
    fn map(index: usize, order: usize, n: usize) -> usize {
        index & (n - 1)
    }

    fn pow2(order: usize) -> usize {
        return 1usize << order;
    }

    /// TODO
    pub fn new_empty(order: usize) -> Self {
        let n = Self::pow2(order + 1);

        let mut entries = Vec::with_capacity(n);
        for _ in 0..n {
            entries.push(atomic::AtomicU64::new(0));
        }

        Self {
            order,
            head: atomic::AtomicUsize::new(0),
            tail: atomic::AtomicUsize::new(0),
            threshold: atomic::AtomicIsize::new(-1),

            entries: Arc::new(entries),
        }
    }
    /// TODO
    pub fn new_full(order: usize) -> Self {
        let half = Self::pow2(order);
        let n = half * 2;

        let mut entries = Vec::with_capacity(n);
        for i in 0..half {
            entries.push(atomic::AtomicU64::new(Self::map(n + i, order, half) as u64));
        }
        for _ in 0..half {
            entries.push(atomic::AtomicU64::new(0));
        }

        Self {
            order,
            head: atomic::AtomicUsize::new(0),
            tail: atomic::AtomicUsize::new(half),
            threshold: atomic::AtomicIsize::new((half + n - 1) as isize),

            entries: Arc::new(entries),
        }
    }

    /// TODO
    pub fn enqueue(&self, index: usize) {
        let n = Self::pow2(self.order) * 2;

        let index = index ^ (n - 1);

        loop {
            let tail = self.tail.fetch_add(1, atomic::Ordering::SeqCst);
            let t_cycle = ((tail << 1) | (2 * n - 1)) as u64;

            let j = Self::map(tail, self.order, n);

            let entry_slot = self.entries.get(j).unwrap();
            let entry = entry_slot.load(atomic::Ordering::SeqCst);

            println!("Tail-Cycle: {}", t_cycle);
            println!("Tail: {}", tail);

            loop {
                let e_cycle = entry | (2 * n - 1) as u64;
                let empty_index = e_cycle ^ (n as u64);

                println!("Entry-Cycle: {}", e_cycle);
                println!("Entry: {}", entry);

                let cycle_cmp = e_cycle < t_cycle;
                let entry_cycle_cmp = entry == e_cycle;
                let entry_empty = entry == empty_index;
                let head_cmp = self.head.load(atomic::Ordering::SeqCst) <= tail;

                println!(
                    "{} && ( {} || ( {} && {} ) )",
                    cycle_cmp, entry_cycle_cmp, entry_empty, head_cmp
                );

                if cycle_cmp && (entry_cycle_cmp || (entry_empty && head_cmp)) {
                    println!("Found store location");
                    match entry_slot.compare_exchange(
                        entry,
                        t_cycle ^ index as u64,
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    ) {
                        Ok(_) => {}
                        Err(_) => {
                            println!("Failed");
                            continue;
                        }
                    };

                    let new_threshold = (3 * n - 1) as isize;
                    if self.threshold.load(atomic::Ordering::SeqCst) != new_threshold {
                        self.threshold
                            .store(new_threshold, atomic::Ordering::SeqCst);
                    }
                    return;
                }

                break;
            }
        }
    }

    fn catchup(&self, mut tail: usize, mut head: usize) {
        loop {
            match self.tail.compare_exchange(
                tail,
                head,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
            ) {
                Ok(_) => {
                    break;
                }
                Err(_) => {}
            };

            head = self.head.load(atomic::Ordering::SeqCst);
            tail = self.tail.load(atomic::Ordering::SeqCst);

            if tail >= head {
                break;
            }
        }
    }

    /// TODO
    pub fn dequeue(&self) -> Option<usize> {
        if self.threshold.load(atomic::Ordering::SeqCst) < 0 {
            return None;
        }

        let n = Self::pow2(self.order) * 2;

        loop {
            let head = self.head.fetch_add(1, atomic::Ordering::SeqCst);
            let h_cycle = ((head << 1) | (2 * n - 1)) as u64;
            let h_idx = head % (n * 2);

            let mut attempt = 0;

            loop {
                let entry_slot = self.entries.get(h_idx).unwrap();
                let entry = entry_slot.load(atomic::Ordering::SeqCst);

                loop {
                    let e_cycle = entry | (2 * n - 1) as u64;
                    if e_cycle == h_cycle as u64 {
                        entry_slot.fetch_or((n - 1) as u64, atomic::Ordering::SeqCst);
                        return Some(entry as usize & (n - 1));
                    }

                    let entry_new;
                    if (entry | n as u64) != e_cycle {
                        entry_new = entry & !n as u64;
                        if entry == entry_new {
                            break;
                        }
                    } else {
                        if attempt <= 10000 {
                            continue;
                        }
                        attempt += 1;
                        entry_new = (h_cycle ^ ((!entry) & (n) as u64)) as u64;
                    }

                    if e_cycle < h_cycle
                        && entry_slot
                            .compare_exchange(
                                entry,
                                entry_new,
                                atomic::Ordering::SeqCst,
                                atomic::Ordering::SeqCst,
                            )
                            .is_err()
                    {
                        continue;
                    } else {
                        break;
                    }
                }
                break;
            }

            let tail = self.tail.load(atomic::Ordering::SeqCst);
            if tail <= head + 1 {
                self.catchup(tail, head + 1);
                self.threshold.fetch_sub(1, atomic::Ordering::SeqCst);
                return None;
            }

            if self.threshold.fetch_sub(1, atomic::Ordering::SeqCst) <= 0 {
                return None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        let queue = Queue::new_empty(2);
        drop(queue);
    }

    #[test]
    fn enqueue() {
        let queue = Queue::new_empty(2);

        queue.enqueue(15);
    }
    #[test]
    #[ignore]
    fn enqueue_dequeue() {
        let queue = Queue::new_empty(2);

        queue.enqueue(15);
        assert_eq!(Some(15), queue.dequeue());
    }
}
