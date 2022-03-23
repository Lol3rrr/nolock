use alloc::{boxed::Box, string::String, vec, vec::Vec};
use core::{
    fmt::Debug,
    marker::{PhantomData, PhantomPinned},
    mem::ManuallyDrop,
};

use super::{
    entry::Entry,
    mptr::{self, boxed_entry, boxed_hashlevel, LoadResult},
    RefValue,
};
use crate::{hash_trie::mptr::PtrType, hyaline, sync::atomic};

pub(crate) struct HashLevel<K, V, const B: u8> {
    /// The Level of the HashLevel, this is used to determine which bits should
    /// be used to lookup the Key/Hash
    pub level: usize,
    /// A Ptr to itself
    pub own: *const HashLevel<K, V, B>,
    /// The Max-Number of Elements that are in a single Chain
    pub max_chain: usize,
    /// A Ptr to the Previous HashLevel
    pub previous: *const HashLevel<K, V, B>,
    /// All the buckets for the current one
    buckets: Vec<mptr::TargetPtr<K, V>>,
    _pin_marker: PhantomPinned,
    _marker: PhantomData<(K, V)>,
}

impl<K, V, const B: u8> HashLevel<K, V, B> {
    /// Creates a new HashLevel
    pub fn new(previous: *const HashLevel<K, V, B>, level: usize) -> Box<Self> {
        let bucket_count = 2usize.pow(B as u32);
        let buckets = Vec::with_capacity(bucket_count);

        let mut result = Box::new(Self {
            level,
            previous,
            max_chain: 3,
            own: core::ptr::null(),
            buckets,
            _pin_marker: PhantomPinned,
            _marker: PhantomData,
        });

        let own_ptr = &*result as *const HashLevel<K, V, B>;
        for _ in 0..bucket_count {
            result
                .buckets
                .push(mptr::TargetPtr::new_hashlevel(own_ptr as *mut Self));
        }

        result.own = own_ptr;

        result
    }

    /// Filters the given Hash according to the current Hash-Level
    fn calc_level_hash(&self, hash: u64) -> u64 {
        debug_assert!(self.level < 64);
        let start = (B as usize) * self.level;
        let end = (B as usize) * (self.level + 1);

        let mask = u64::MAX.wrapping_shl(start as u32) >> start;
        (hash & mask) >> (64 - end)
    }

    /// Calculates the Index of the Bucket for a given Hash
    fn get_bucket_index(&self, hash: u64) -> usize {
        self.calc_level_hash(hash) as usize
    }

    pub fn get_bucket(&self, hash: u64) -> Option<&mptr::TargetPtr<K, V>> {
        let index = self.get_bucket_index(hash);
        self.buckets.get(index)
    }

    pub fn cleanup_buckets(&mut self, handle: &mut hyaline::Handle<'_>) {
        for bucket in self.buckets.iter() {
            match bucket.load_ptr(atomic::Ordering::SeqCst) {
                PtrType::Entry(ptr) => {
                    bucket.raw_store(self.own as *mut (), atomic::Ordering::SeqCst);

                    Entry::clean_up::<B>(ptr as *mut Entry<K, V>, self.own as *mut (), handle);
                }
                PtrType::HashLevel(ptr) => {
                    if ptr == self.own as *mut () {
                        continue;
                    }

                    let level = unsafe { &mut *(ptr as *mut Self) };
                    level.cleanup_buckets(handle);
                }
            };
        }

        // TODO
        // todo!("Cleanup buckets")
    }
}

impl<K, V, const B: u8> HashLevel<K, V, B>
where
    K: Eq,
{
    /// Attempts to append the Node `n` to the chain of Node `r`. Additionally
    /// this might cause the allocation of a new HashLevel
    fn adjust_node_on_chain(&self, n: &Entry<K, V>, r: &Entry<K, V>, chain: usize) {
        // Load the Next-Element in the Chain and if it is Hashlevel
        if let LoadResult::HashLevel { ptr: hash_ptr, .. } = r.other.load::<B>() {
            // If the current chain already has the Maximum length, create
            // a new HashLevel and then move all the Nodes in the Chain
            // to the new HashLevel as well as then inserting the Node `n`
            // into the new HashLevel
            if chain == self.max_chain {
                let new_hash = HashLevel::new(self.own, self.level + 1);
                let new_hash_ptr = Box::into_raw(new_hash);

                let cas_ptr = mptr::mark_as_previous(hash_ptr as *const u8) as *mut Entry<K, V>;
                match r.other.cas_hashlevel::<B>(
                    cas_ptr,
                    new_hash_ptr as *mut (),
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        let new_hash = boxed_hashlevel(new_hash_ptr);

                        let bucket_index = self.get_bucket_index(n.hash);
                        let bucket = self.buckets.get(bucket_index).unwrap();

                        match bucket.load::<B>() {
                            LoadResult::Entry { entry, .. } => {
                                new_hash.adjust_chain_nodes(entry);
                            }
                            _ => {
                                // println!("Expected Bucket to point to Entry");
                                return;
                            }
                        };

                        bucket.store_hashlevel(new_hash_ptr as *mut (), atomic::Ordering::SeqCst);

                        return;
                    }
                    Err(_) => {
                        // println!("Failed CAS");
                    }
                };
            } else {
                // We have reached the End of the Chain, so we should attempt
                // to simply add the new None to the End of the Chain
                let n_ptr: *const Entry<K, V> = n;
                let cas_ptr = mptr::mark_as_previous(hash_ptr as *const u8) as *mut Entry<K, V>;
                match r.other.cas_entry::<B>(
                    cas_ptr,
                    n_ptr as *mut (),
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => return,
                    Err(_) => {
                        // Something modified the Next-Ptr before us, so we
                        // should "retry"
                    }
                }
            }
        }

        // Load the next Element in the Chain
        match r.other.load::<B>() {
            // If the next Element is also an Entry, call this function
            // recursively with the next Entry as the Chain "root"
            LoadResult::Entry { entry, .. } => {
                self.adjust_node_on_chain(n, entry, chain + 1);
            }
            // If the next Element is a HashLevel, try and insert the node
            // in the next HashLevel after this one
            LoadResult::HashLevel { level: mut r, .. } => {
                // Go back into the previous HashLevel, until you find the
                // HashLevel, directly "below" the current HashLevel
                while r.previous != self.own {
                    r = unsafe { &*r.previous };
                }

                r.adjust_node_on_hash(n);
            }
        };
    }

    /// Adjusts the Node to fit into the current HashLevel
    fn adjust_node_on_hash(&self, n: &Entry<K, V>) {
        // Set the Next-Element to be the current HashLevel
        n.other
            .store_hashlevel(self.own as *mut (), atomic::Ordering::SeqCst);

        // Find the corresponding Bucket for the given Node
        let bucket_index = self.get_bucket_index(n.hash);
        let bucket = self.buckets.get(bucket_index).unwrap();

        // If the Bucket Points to the current HashLevel, the bucket
        // is empty and we can simply CAS the new Node into the Bucket
        if let LoadResult::HashLevel { ptr: level_ptr, .. } = bucket.load::<B>() {
            if level_ptr == self.own as *mut Self {
                let n_ptr: *const Entry<K, V> = n;

                let marked = mptr::mark_as_previous(level_ptr as *const u8) as *mut u8;
                match bucket.cas_entry::<B>(
                    marked as *mut Entry<K, V>,
                    n_ptr as *mut (),
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        return;
                    }
                    Err(_) => {
                        // println!("Insert did not work");
                    }
                };
            }
        }

        // Load the bucket Entry again
        match bucket.load::<B>() {
            // Bucket already contains a Node
            LoadResult::Entry { entry, .. } => {
                self.adjust_node_on_chain(n, entry, 1);
            }
            // Bucket points to a second HashLevel so we should
            // try and adjust the Node "onto" the newly found
            // HashLevel
            LoadResult::HashLevel { level: r, .. } => {
                r.adjust_node_on_hash(n);
            }
        };
    }

    /// Starts the adjustment process for the given Node as well as starting
    /// the adjustment for all the Nodes in its Chain
    pub fn adjust_chain_nodes(&self, r: &Entry<K, V>) {
        if let LoadResult::Entry { entry, .. } = r.other.load::<B>() {
            self.adjust_chain_nodes(entry);
        }
        self.adjust_node_on_hash(r);
    }

    /// Inserts the new Entry into the current HashLevel
    pub fn insert_key_on_hash(
        &self,
        hash: u64,
        key: K,
        value: V,
        handle: &mut hyaline::Handle<'_>,
    ) {
        let bucket = self.buckets.get(self.get_bucket_index(hash)).expect(
            "The Bucket should always exist as there Hash should never be bigger than 2^bits",
        );

        let mut new_entry = ManuallyDrop::new(Entry::new_hashlevel(
            hash,
            key,
            value,
            self.own as *mut Self,
        ));

        // If the
        if let mptr::PtrType::HashLevel(bucket_ptr) = bucket.load_ptr(atomic::Ordering::Acquire) {
            let bucket_ptr = bucket_ptr as *mut Self;
            if bucket_ptr == self.own as *mut Self {
                let n_ptr = Box::into_raw(ManuallyDrop::into_inner(new_entry));
                let cas_ptr = mptr::mark_as_previous(self.own as *const u8) as *mut Entry<K, V>;

                match bucket.cas_entry::<B>(
                    cas_ptr,
                    n_ptr as *mut (),
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => return,
                    Err(_) => {
                        new_entry = boxed_entry(n_ptr);
                    }
                };
            }
        }

        match bucket.load::<B>() {
            LoadResult::HashLevel { level: sub_lvl, .. } => {
                let raw_new_entry = ManuallyDrop::into_inner(new_entry);

                sub_lvl.insert_key_on_hash(
                    raw_new_entry.hash,
                    raw_new_entry.key,
                    raw_new_entry.value,
                    handle,
                )
            }
            LoadResult::Entry { entry, .. } => {
                entry.insert_key_on_chain(hash, &self, new_entry, 1, handle)
            }
        }
    }

    pub fn insert(&self, hash: u64, key: K, value: V, handle: &mut hyaline::Handle<'_>) {
        self.insert_key_on_hash(hash, key, value, handle);
    }

    pub fn get<'a>(
        &self,
        hash: u64,
        key: &K,
        handle: hyaline::Handle<'a>,
    ) -> Option<RefValue<'a, K, V>> {
        let bucket_index = self.get_bucket_index(hash);
        let bucket = self.buckets.get(bucket_index).expect(
            "The Bucket should always exist as there Hash should never be bigger than 2^bits",
        );

        if let mptr::PtrType::HashLevel(h_ptr) = bucket.load_ptr(atomic::Ordering::Acquire) {
            if h_ptr as *mut Self == self.own as *mut Self {
                return None;
            }
        }

        match bucket.load::<B>() {
            LoadResult::Entry { entry, .. } => match entry.get_chain(hash, &self, key, 1, handle) {
                Ok(v) => Some(v),
                _ => None,
            },
            LoadResult::HashLevel { level: sub_lvl, .. } => sub_lvl.get(hash, key, handle),
        }
    }

    fn invalidate_entry(&self, hash: u64, key: &K) {
        let bucket = self.get_bucket(hash).unwrap();

        match bucket.load::<B>() {
            LoadResult::HashLevel {
                level: sub_lvl,
                ptr: sub_lvl_ptr,
            } => {
                if self.own == sub_lvl_ptr {
                    return;
                }

                sub_lvl.invalidate_entry(hash, key);
            }
            LoadResult::Entry {
                entry: mut current_entry,
                ..
            } => loop {
                if &current_entry.key == key {
                    current_entry.invalidate(atomic::Ordering::SeqCst);
                    return;
                }

                match current_entry.other.load::<B>() {
                    LoadResult::HashLevel {
                        level: sub_lvl,
                        ptr: sub_lvl_ptr,
                    } => {
                        if self.own == sub_lvl_ptr {
                            return;
                        }
                        sub_lvl.invalidate_entry(hash, key);
                        break;
                    }
                    LoadResult::Entry {
                        entry: next_entry, ..
                    } => {
                        current_entry = next_entry;
                    }
                };
            },
        };
    }

    fn remove_entry_chain(
        previous: &mptr::TargetPtr<K, V>,
        to_remove: &Entry<K, V>,
        handle: &mut hyaline::Handle<'_>,
    ) {
        let mut next_ptr = to_remove.other.raw_load(atomic::Ordering::SeqCst);
        loop {
            previous.raw_store(next_ptr, atomic::Ordering::SeqCst);
            let tmp = to_remove.other.raw_load(atomic::Ordering::SeqCst);
            if next_ptr == tmp {
                break;
            }
            next_ptr = tmp;
        }

        let retire_ptr: *const Entry<K, V> = to_remove;
        unsafe {
            handle.retire(retire_ptr as *const ());
        }

        return;
    }

    fn invisible_entry(&self, hash: u64, key: &K, handle: &mut hyaline::Handle<'_>) {
        let bucket = self.get_bucket(hash).unwrap();

        match bucket.load::<B>() {
            LoadResult::HashLevel {
                level: sub_lvl,
                ptr: sub_lvl_ptr,
            } => {
                if self.own == sub_lvl_ptr {
                    return;
                }

                sub_lvl.invisible_entry(hash, key, handle);
            }
            LoadResult::Entry { mut entry, .. } => {
                if &entry.key == key {
                    Self::remove_entry_chain(&bucket, entry, handle);

                    return;
                }

                loop {
                    match entry.other.load::<B>() {
                        LoadResult::HashLevel {
                            level: sub_lvl,
                            ptr: sub_lvl_ptr,
                        } => {
                            if self.own == sub_lvl_ptr {
                                return;
                            }
                            sub_lvl.invisible_entry(hash, key, handle);
                            break;
                        }
                        LoadResult::Entry {
                            entry: next_entry, ..
                        } => {
                            if &next_entry.key == key {
                                Self::remove_entry_chain(&entry.other, next_entry, handle);
                                return;
                            }

                            entry = next_entry;
                        }
                    };
                }
            }
        };
    }

    pub fn remove_entry<'h>(&self, hash: u64, key: &K, handle: &mut hyaline::Handle<'h>) {
        self.invalidate_entry(hash, key);
        self.invisible_entry(hash, key, handle);
    }
}

impl<K, V, const B: u8> Debug for HashLevel<K, V, B>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let padding = String::from_utf8(vec![b' '; self.level + 1]).unwrap();

        writeln!(f, "{}Own: {:p}", padding, self.own)?;
        /*
        let mut bucket_guard = self.domain.empty_guard();
        for bucket in self.buckets.iter() {
            match bucket.load::<B>(&mut bucket_guard) {
                None => {
                    writeln!(f, "{}{:?}", padding, bucket_guard)?;
                }
                Some((sub_lvl, hashlvl_ptr)) if hashlvl_ptr != self.own as *mut Self => {
                    writeln!(f, "{}HashLevel:", padding)?;
                    write!(f, "{:?}", sub_lvl)?;
                    std::mem::forget(sub_lvl);
                }
                Some((_, hashlvl_ptr)) if hashlvl_ptr == self.own as *mut Self => {
                    writeln!(f, "{}Empty", padding)?;
                }
                _ => {}
            };
        }*/
        Ok(())
    }
}

impl<K, V, const B: u8> Drop for HashLevel<K, V, B> {
    fn drop(&mut self) {
        for bucket in self.buckets.iter() {
            match bucket.load_ptr(atomic::Ordering::SeqCst) {
                PtrType::Entry(_) => {}
                PtrType::HashLevel(level_ptr) => {
                    if level_ptr == self.own as *mut () {
                        continue;
                    }

                    let _ = unsafe { Box::from_raw(level_ptr as *mut Self) };
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::RandomState;

    use crate::hash_trie::HashTrieMap;

    use super::*;

    #[test]
    fn hash_level_calc_hash() {
        let hl_0 = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        assert_eq!(0x01, hl_0.calc_level_hash(0x1234567890abcdef));

        let hl_1 = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 1);
        assert_eq!(0x02, hl_1.calc_level_hash(0x1234567890abcdef));
    }

    #[test]
    fn hash_level_insert_get() {
        let instance = hyaline::Hyaline::<4>::new(HashTrieMap::<u64, u64, RandomState>::free_func);
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        let hash = 13;
        let key = 16;
        let value = 123;
        hl.insert(hash, key, value, &mut instance.enter());

        assert_eq!(hl.get(hash, &16, instance.enter()).unwrap(), value);
    }
    #[test]
    fn hash_level_insert_get_collision() {
        let instance = hyaline::Hyaline::<4>::new(HashTrieMap::<u64, u64, RandomState>::free_func);
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        let hash = 13;
        let key = 16;
        let value = 123;
        hl.insert(hash, key, value, &mut instance.enter());

        hl.insert(hash, 17, 124, &mut instance.enter());

        assert_eq!(hl.get(hash, &17, instance.enter()).unwrap(), 124);
    }

    #[test]
    fn hash_level_insert_collision_expand() {
        let instance = hyaline::Hyaline::<4>::new(HashTrieMap::<u64, u64, RandomState>::free_func);
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        hl.insert(0x1234567890abcdef, 13, 123, &mut instance.enter()); // First: 0x1 Second: 0x2
        hl.insert(0x1234567890abcdef, 14, 124, &mut instance.enter()); // First: 0x1 Second: 0x2
        hl.insert(0x1334567890abcdef, 15, 125, &mut instance.enter()); // First: 0x1 Second: 0x3
        hl.insert(0x1334567890abcdef, 16, 126, &mut instance.enter()); // First: 0x1 Second: 0x3

        assert_eq!(
            hl.get(0x1234567890abcdef, &13, instance.enter()).unwrap(),
            123
        );
        assert_eq!(
            hl.get(0x1234567890abcdef, &14, instance.enter()).unwrap(),
            124
        );
        assert_eq!(
            hl.get(0x1334567890abcdef, &15, instance.enter()).unwrap(),
            125
        );
        assert_eq!(
            hl.get(0x1334567890abcdef, &16, instance.enter()).unwrap(),
            126
        );
    }

    #[test]
    fn insert_remove() {
        let instance = hyaline::Hyaline::<4>::new(HashTrieMap::<u64, u64, RandomState>::free_func);
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        hl.insert(0x1234567890abcdef, 13, 123, &mut instance.enter());
        assert_eq!(
            true,
            hl.get(0x1234567890abcdef, &13, instance.enter()).is_some()
        );

        hl.remove_entry(0x1234567890abcdef, &13, &mut instance.enter());
        assert_eq!(
            false,
            hl.get(0x1234567890abcdef, &13, instance.enter()).is_some()
        );
    }
    #[test]
    fn insert_remove_chain() {
        let instance = hyaline::Hyaline::<4>::new(HashTrieMap::<u64, u64, RandomState>::free_func);
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        hl.insert(0x1234567890abcdef, 13, 123, &mut instance.enter());
        hl.insert(0x1234567890abcdef, 14, 124, &mut instance.enter());
        assert_eq!(
            true,
            hl.get(0x1234567890abcdef, &13, instance.enter()).is_some()
        );
        assert_eq!(
            true,
            hl.get(0x1234567890abcdef, &14, instance.enter()).is_some()
        );

        hl.remove_entry(0x1234567890abcdef, &14, &mut instance.enter());
        assert_eq!(
            true,
            hl.get(0x1234567890abcdef, &13, instance.enter()).is_some()
        );
        assert_eq!(
            false,
            hl.get(0x1234567890abcdef, &14, instance.enter()).is_some()
        );
    }
}
