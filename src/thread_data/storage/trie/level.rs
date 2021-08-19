use std::sync::atomic;

use crate::thread_data::storage::trie::{Entry, PtrTarget};

use super::CustomPtr;

#[derive(Debug)]
pub struct Level<T> {
    level: usize,
    previous: *const Self,
    entries: Vec<CustomPtr<T>>,
    key_size: usize,
}

impl<T> Level<T> {
    pub fn new(level: usize, key_size: usize, previous: *const Self) -> Box<Self> {
        let bucket_count = 2usize.pow(key_size as u32);
        let mut result = Box::new(Self {
            level,
            previous,
            entries: Vec::with_capacity(bucket_count),
            key_size,
        });

        let own_ptr = &*result as *const Self as *mut Self;
        for _ in 0..bucket_count {
            result.entries.push(CustomPtr::new_level(own_ptr));
        }

        result
    }

    pub fn max_chain(&self) -> usize {
        2
    }
    pub fn level(&self) -> usize {
        self.level
    }
    pub fn key_size(&self) -> usize {
        self.key_size
    }
    pub fn get_own_ptr(&self) -> *mut Self {
        self as *const Self as *mut Self
    }
    pub fn previous(&self) -> *mut Self {
        self.previous as *mut Self
    }

    fn index(key: u64, level: usize, key_size: usize) -> usize {
        let start = key_size * level;
        let end = key_size * (level + 1);
        let mask = !(u64::MAX << end);
        ((key & mask) >> start) as usize
    }

    fn adjust_node_on_chain(&self, node: &Entry<T>, chain: &Entry<T>, chain_pos: usize) {
        if let PtrTarget::Level(sub_lvl_ptr) = chain.next.load(atomic::Ordering::Acquire) {
            if chain_pos == self.max_chain() {
                let new_level = Level::new(self.level + 1, self.key_size, self.get_own_ptr());
                let new_level_ptr = Box::into_raw(new_level);

                if chain
                    .next
                    .compare_exchange(
                        PtrTarget::Level(sub_lvl_ptr),
                        PtrTarget::Level(new_level_ptr),
                        atomic::Ordering::AcqRel,
                        atomic::Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    let new_level = unsafe { &*new_level_ptr };

                    let bucket_index = Self::index(node.key(), self.level, self.key_size);
                    let bucket = self.entries.get(bucket_index).expect("");

                    match bucket.load(atomic::Ordering::Acquire) {
                        PtrTarget::Entry(bucket_entry_ptr) => {
                            let bucket_entry = unsafe { &*bucket_entry_ptr };
                            new_level.adjust_chain_nodes(bucket_entry);
                        }
                        _ => unreachable!(),
                    };

                    bucket.store(PtrTarget::Level(new_level_ptr), atomic::Ordering::Release);

                    return;
                }
            } else {
                let node_ptr = node as *const Entry<T> as *mut Entry<T>;

                if chain
                    .next
                    .compare_exchange(
                        PtrTarget::Level(sub_lvl_ptr),
                        PtrTarget::Entry(node_ptr),
                        atomic::Ordering::AcqRel,
                        atomic::Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return;
                }
            }
        }

        match chain.next.load(atomic::Ordering::Acquire) {
            PtrTarget::Entry(next_entry_ptr) => {
                let next_entry = unsafe { &*next_entry_ptr };
                self.adjust_node_on_chain(node, next_entry, chain_pos + 1);
            }
            PtrTarget::Level(sub_lvl_ptr) => {
                let mut sub_lvl = unsafe { &*sub_lvl_ptr };

                while sub_lvl.previous != self.get_own_ptr() {
                    sub_lvl = unsafe { &*sub_lvl.previous() };
                }

                sub_lvl.adjust_node_on_level(node);
            }
        };
    }

    fn adjust_node_on_level(&self, node: &Entry<T>) {
        node.next.store(
            PtrTarget::Level(self.get_own_ptr()),
            atomic::Ordering::Release,
        );

        let bucket_index = Self::index(node.key(), self.level, self.key_size);
        let bucket = self.entries.get(bucket_index).expect("");

        if let PtrTarget::Level(sub_lvl_ptr) = bucket.load(atomic::Ordering::Acquire) {
            let sub_lvl = unsafe { &*sub_lvl_ptr };

            if sub_lvl.level() == self.level() {
                let node_ptr = node as *const Entry<T> as *mut Entry<T>;

                if bucket
                    .compare_exchange(
                        PtrTarget::Level(self.get_own_ptr()),
                        PtrTarget::Entry(node_ptr),
                        atomic::Ordering::AcqRel,
                        atomic::Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    return;
                }
            }
        }

        match bucket.load(atomic::Ordering::Acquire) {
            PtrTarget::Entry(entry_ptr) => {
                let chain_entry = unsafe { &*entry_ptr };
                self.adjust_node_on_chain(node, chain_entry, 1);
            }
            PtrTarget::Level(sub_lvl_ptr) => {
                let sub_lvl = unsafe { &*sub_lvl_ptr };
                sub_lvl.adjust_node_on_level(node);
            }
        };
    }

    fn adjust_chain_nodes(&self, node: &Entry<T>) {
        if let PtrTarget::Entry(entry_ptr) = node.next.load(atomic::Ordering::Acquire) {
            let entry = unsafe { &*entry_ptr };
            self.adjust_chain_nodes(entry);
        }
        self.adjust_node_on_level(node);
    }

    pub fn move_buckets_to_new_level(&self, key: u64, n_level_ptr: *mut Self) {
        let bucket_index = Self::index(key, self.level, self.key_size);
        let bucket = self.entries.get(bucket_index).expect("");

        let initial_entry = match bucket.load(atomic::Ordering::Acquire) {
            PtrTarget::Entry(entry_ptr) => unsafe { &*entry_ptr },
            _ => unreachable!(),
        };

        let n_level = unsafe { &*n_level_ptr };
        n_level.adjust_chain_nodes(initial_entry);

        bucket.store(PtrTarget::Level(n_level_ptr), atomic::Ordering::Release);
    }

    pub fn insert_level(&self, mut new_entry: Box<Entry<T>>) -> &T {
        let bucket_index = Self::index(new_entry.key(), self.level, self.key_size);
        let bucket = self.entries.get(bucket_index).expect("");

        if let PtrTarget::Level(sub_lvl_ptr) = bucket.load(atomic::Ordering::Acquire) {
            let sub_lvl = unsafe { &*sub_lvl_ptr };

            if sub_lvl.level == self.level {
                new_entry.next.store(
                    PtrTarget::Level(self.get_own_ptr()),
                    atomic::Ordering::Release,
                );
                let new_entry_ptr = Box::into_raw(new_entry);

                match bucket.compare_exchange(
                    PtrTarget::Level(sub_lvl_ptr),
                    PtrTarget::Entry(new_entry_ptr),
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let entry = unsafe { &*new_entry_ptr };
                        return entry.data();
                    }
                    Err(_) => {
                        new_entry = unsafe { Box::from_raw(new_entry_ptr) };
                    }
                };
            }
        }

        match bucket.load(atomic::Ordering::Acquire) {
            PtrTarget::Level(sub_lvl_ptr) => {
                let sub_lvl = unsafe { &*sub_lvl_ptr };
                sub_lvl.insert_level(new_entry)
            }
            PtrTarget::Entry(entry_ptr) => {
                let entry = unsafe { &*entry_ptr };
                entry.insert_chain(new_entry, self)
            }
        }
    }

    pub fn insert(&self, key: u64, data: T) -> &T {
        let entry = Box::new(Entry::new(
            key,
            data,
            CustomPtr::new_level(self.get_own_ptr()),
        ));
        self.insert_level(entry)
    }

    pub fn get(&self, key: u64) -> Option<&T> {
        let bucket_index = Self::index(key, self.level, self.key_size);

        let bucket = self
            .entries
            .get(bucket_index)
            .expect("The Bucket-Index is always within the List of Buckets");

        match bucket.load(atomic::Ordering::SeqCst) {
            PtrTarget::Level(level_ptr) => {
                let level = unsafe { &*level_ptr };
                if level.level == self.level {
                    // The Level pointed to is at the Same level in the Hierachy
                    // as the current Level, meaning that we point at ourselves
                    // as any other Pointer would only point to something further
                    // down the Hierachy which would have a different Level
                    return None;
                }

                level.get(key)
            }
            PtrTarget::Entry(entry_ptr) => {
                let entry = unsafe { &*entry_ptr };
                entry.get_chain(key, self)
            }
        }
    }
}

impl<T> Drop for Level<T> {
    fn drop(&mut self) {
        let current_level_ptr = self.get_own_ptr();

        for entries in self.entries.drain(..) {
            match entries.load(atomic::Ordering::SeqCst) {
                PtrTarget::Entry(entry_ptr) => {
                    let boxed = unsafe { Box::from_raw(entry_ptr) };
                    boxed.drop_entry(current_level_ptr);
                }
                PtrTarget::Level(level_ptr) => {
                    if level_ptr == current_level_ptr {
                        continue;
                    }

                    unsafe { Box::from_raw(level_ptr) };
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        Level::<usize>::new(0, 4, std::ptr::null());
    }

    #[test]
    fn level() {
        assert_eq!(0x34, Level::<usize>::index(0x1234, 0, 8));
        assert_eq!(0x12, Level::<usize>::index(0x1234, 1, 8));

        assert_eq!(0x4, Level::<usize>::index(0x1234, 0, 4));
        assert_eq!(0x3, Level::<usize>::index(0x1234, 1, 4));
        assert_eq!(0x2, Level::<usize>::index(0x1234, 2, 4));
        assert_eq!(0x1, Level::<usize>::index(0x1234, 3, 4));
    }
}
