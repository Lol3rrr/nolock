//! A lock-free concurrent HashTrieMap
//!
//! # Reference:
//! * [A Lock-Free Hash Trie Design for Concurrent Tabled Logic Programs](https://link.springer.com/content/pdf/10.1007/s10766-014-0346-1.pdf)

use std::{
    collections::hash_map::RandomState,
    fmt::Debug,
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::ManuallyDrop,
    sync::atomic,
};

use crate::hash_trie::mptr::boxed_hashlevel;

use self::mptr::boxed_entry;

mod mptr {
    use super::{Entry, HashLevel};
    use std::{mem::ManuallyDrop, sync::atomic};

    pub(crate) enum PtrTarget<K, V> {
        HashLevel(*mut HashLevel<K, V>),
        Entry(*mut Entry<K, V>),
    }

    pub(crate) fn boxed_hashlevel<K, V>(
        ptr: *mut HashLevel<K, V>,
    ) -> ManuallyDrop<Box<HashLevel<K, V>>> {
        let boxed = unsafe { Box::from_raw(ptr) };
        ManuallyDrop::new(boxed)
    }
    pub(crate) fn boxed_entry<K, V>(ptr: *mut Entry<K, V>) -> ManuallyDrop<Box<Entry<K, V>>> {
        let boxed = unsafe { Box::from_raw(ptr) };
        ManuallyDrop::new(boxed)
    }

    pub(crate) struct Atomic {
        ptr: atomic::AtomicPtr<u8>,
    }

    impl Atomic {
        pub fn new<T>(ptr: *mut T) -> Self {
            Self {
                ptr: atomic::AtomicPtr::new(ptr as *mut u8),
            }
        }

        pub fn load<K, V>(&self, order: atomic::Ordering) -> PtrTarget<K, V> {
            let raw = self.ptr.load(order);

            if is_entry(raw as *const u8) {
                PtrTarget::Entry(to_actual_ptr(raw as *const u8) as *mut Entry<K, V>)
            } else {
                PtrTarget::HashLevel(to_actual_ptr(raw as *const u8) as *mut HashLevel<K, V>)
            }
        }
        pub fn store_hashlevel<K, V>(&self, ptr: *mut HashLevel<K, V>, order: atomic::Ordering) {
            let marked = mark_as_previous(ptr as *const u8) as *mut u8;
            self.ptr.store(marked, order);
        }
        pub fn store_entry<K, V>(&self, ptr: *mut Entry<K, V>, order: atomic::Ordering) {
            let marked = mark_as_entry(ptr as *const u8) as *mut u8;
            self.ptr.store(marked, order);
        }

        pub fn cas_hashlevel<K, V>(
            &self,
            current: *mut u8,
            new: *mut HashLevel<K, V>,
            success: atomic::Ordering,
            failure: atomic::Ordering,
        ) -> Result<*mut u8, *mut u8> {
            let marked = mark_as_previous(new as *const u8) as *mut u8;
            self.ptr.compare_exchange(current, marked, success, failure)
        }
        pub fn cas_entry<K, V>(
            &self,
            current: *mut u8,
            new: *mut Entry<K, V>,
            success: atomic::Ordering,
            failure: atomic::Ordering,
        ) -> Result<*mut u8, *mut u8> {
            let marked = mark_as_entry(new as *const u8) as *mut u8;
            self.ptr.compare_exchange(current, marked, success, failure)
        }
    }

    pub fn is_previous(ptr: *const u8) -> bool {
        (ptr as usize) & 0x01 == 1
    }
    pub fn is_entry(ptr: *const u8) -> bool {
        (ptr as usize) & 0x01 == 0
    }

    pub fn mark_as_previous(ptr: *const u8) -> *const u8 {
        ((ptr as usize) | 0x01) as *const u8
    }
    pub fn mark_as_entry(ptr: *const u8) -> *const u8 {
        ((ptr as usize) | 0x00) as *const u8
    }

    pub fn to_actual_ptr(ptr: *const u8) -> *const u8 {
        ((ptr as usize) & (usize::MAX - 1)) as *const u8
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn is_checks() {
            assert_eq!(true, is_previous(0xFFFFFFFFFFFFFFF1 as *const u8));
            assert_eq!(false, is_previous(0xFFFFFFFFFFFFFFF0 as *const u8));
            assert_eq!(true, is_entry(0xFFFFFFFFFFFFFFF0 as *const u8));
            assert_eq!(false, is_entry(0xFFFFFFFFFFFFFFF1 as *const u8));
        }

        #[test]
        fn mark_ptrs() {
            assert_eq!(
                0xFFFFFFFFFFFFFFF1 as *const u8,
                mark_as_previous(0xFFFFFFFFFFFFFFF0 as *const u8)
            );
            assert_eq!(
                0xFFFFFFFFFFFFFFF0 as *const u8,
                mark_as_entry(0xFFFFFFFFFFFFFFF0 as *const u8)
            );
        }

        #[test]
        fn to_original() {
            assert_eq!(
                0xFFFFFFFFFFFFFFF0 as *const u8,
                to_actual_ptr(0xFFFFFFFFFFFFFFF1 as *const u8)
            );
            assert_eq!(
                0xFFFFFFFFFFFFFFF0 as *const u8,
                to_actual_ptr(0xFFFFFFFFFFFFFFF0 as *const u8)
            );
        }
    }
}

/// A Concurrent and Lock-Free HashTrieMap
pub struct HashTrieMap<K, V, H = RandomState> {
    initial_level: HashLevel<K, V>,
    _marker: PhantomData<H>,
}

impl<K, V, RandomState> HashTrieMap<K, V, RandomState>
where
    K: Eq + Debug,
    V: Clone + Debug,
{
    /// Creates a new HashTrieMap
    pub fn new() -> Self {
        let start_level = HashLevel::new(0 as *const HashLevel<K, V>, 4, 0);
        Self {
            initial_level: *start_level,
            _marker: PhantomData,
        }
    }
}

impl<K, V, H> HashTrieMap<K, V, H>
where
    K: Hash + Eq + Debug,
    H: Hasher + Default,
    V: Clone + Debug,
{
    /// Inserts the given Key and Value into the Map
    pub fn insert(&self, key: K, value: V) {
        let mut hasher = H::default();
        key.hash(&mut hasher);
        let hash = hasher.finish();

        self.initial_level.insert(hash, key, value);
    }
}

pub(crate) struct HashLevel<K, V> {
    bits: u8,
    level: usize,
    own: *const HashLevel<K, V>,
    max_chain: usize,
    previous: *const HashLevel<K, V>,
    buckets: Vec<mptr::Atomic>,
    _marker: PhantomData<(K, V)>,
}

impl<K, V> HashLevel<K, V>
where
    K: Eq + Debug,
    V: Clone + Debug,
{
    pub fn new(previous: *const HashLevel<K, V>, bits: u8, level: usize) -> Box<Self> {
        let bucket_count = 2usize.pow(bits as u32);
        let buckets = Vec::with_capacity(bucket_count);

        let mut result = Box::new(Self {
            bits,
            level,
            previous,
            max_chain: 3,
            own: 0 as *const HashLevel<K, V>,
            buckets,
            _marker: PhantomData,
        });

        let own_ptr = &*result as *const HashLevel<K, V>;
        let hashlevel_ptr = mptr::mark_as_previous(own_ptr as *const u8) as *mut u8;
        for _ in 0..bucket_count {
            result.buckets.push(mptr::Atomic::new(hashlevel_ptr));
        }

        result.own = own_ptr;

        result
    }

    fn calc_level_hash(&self, hash: u64) -> u64 {
        let start = (self.bits as usize) * self.level;
        let end = (self.bits as usize) * (self.level + 1);

        let mask = (u64::MAX << start) >> start;
        (hash & mask) >> (64 - end)
    }

    fn adjust_node_on_chain(
        &self,
        mut n: ManuallyDrop<Box<Entry<K, V>>>,
        r: ManuallyDrop<Box<Entry<K, V>>>,
        chain: usize,
    ) {
        if let mptr::PtrTarget::HashLevel(hash_ptr) = r.other.load::<K, V>(atomic::Ordering::SeqCst)
        {
            if chain == self.max_chain {
                let new_hash = HashLevel::new(self.own, self.bits, self.level + 1);
                let new_hash_ptr = Box::into_raw(new_hash);

                let cas_ptr = mptr::mark_as_previous(hash_ptr as *const u8) as *mut u8;
                match r.other.cas_hashlevel(
                    cas_ptr,
                    new_hash_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        let new_hash = boxed_hashlevel(new_hash_ptr);

                        let bucket_index = self.get_bucket_index(n.hash);
                        let bucket = self.buckets.get(bucket_index).unwrap();

                        match bucket.load(atomic::Ordering::SeqCst) {
                            mptr::PtrTarget::Entry(entry_ref_ptr) => {
                                let bucket_entry = boxed_entry(entry_ref_ptr);
                                new_hash.adjust_chain_nodes(bucket_entry);
                            }
                            _ => {
                                println!("Expected Bucket to point to Entry");
                                return;
                            }
                        };

                        bucket.store_hashlevel(new_hash_ptr, atomic::Ordering::SeqCst);

                        return;
                    }
                    Err(_) => {
                        println!("Failed CAS");
                    }
                };
            } else {
                let n_ptr = Box::into_raw(ManuallyDrop::into_inner(n));
                let cas_ptr = mptr::mark_as_previous(hash_ptr as *const u8) as *mut u8;
                match r.other.cas_entry(
                    cas_ptr,
                    n_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => return,
                    Err(_) => {
                        n = boxed_entry(n_ptr);
                    }
                }
            }
        }

        match r.other.load(atomic::Ordering::SeqCst) {
            mptr::PtrTarget::Entry(r) => {
                let r = boxed_entry(r);
                self.adjust_node_on_chain(n, r, chain + 1);
            }
            mptr::PtrTarget::HashLevel(r) => {
                let mut r = boxed_hashlevel(r);
                while r.previous != self.own {
                    r = boxed_hashlevel(r.previous as *mut HashLevel<K, V>);
                }

                self.adjust_node_on_hash(n)
            }
        };
    }

    fn adjust_node_on_hash(&self, mut n: ManuallyDrop<Box<Entry<K, V>>>) {
        n.other
            .store_hashlevel(self.own as *mut HashLevel<K, V>, atomic::Ordering::SeqCst);

        let bucket_index = self.get_bucket_index(n.hash);
        let bucket = self.buckets.get(bucket_index).unwrap();

        if let mptr::PtrTarget::HashLevel(level_ptr) = bucket.load::<K, V>(atomic::Ordering::SeqCst)
        {
            let n_ptr = Box::into_raw(ManuallyDrop::into_inner(n));

            let marked = mptr::mark_as_previous(level_ptr as *const u8) as *mut u8;
            match bucket.cas_entry(
                marked,
                n_ptr,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
            ) {
                Ok(_) => {
                    println!("Insert Worked");
                    return;
                }
                Err(_) => {
                    println!("Insert did not work");
                    n = boxed_entry(n_ptr);
                }
            };
        }

        match bucket.load::<K, V>(atomic::Ordering::SeqCst) {
            mptr::PtrTarget::Entry(r) => {
                println!("Bucket has Entry");
                let r = mptr::boxed_entry(r);
                self.adjust_node_on_chain(n, r, 1);
            }
            mptr::PtrTarget::HashLevel(r) => {
                println!("Bucket has HashLevel");
                let r = mptr::boxed_hashlevel(r);
                r.adjust_node_on_hash(n);
            }
        };
    }

    fn adjust_chain_nodes(&self, r: ManuallyDrop<Box<Entry<K, V>>>) {
        match r.other.load(atomic::Ordering::SeqCst) {
            mptr::PtrTarget::Entry(r) => {
                let r = boxed_entry(r);
                self.adjust_chain_nodes(r);
            }
            _ => {}
        };
        self.adjust_node_on_hash(r);
    }

    fn get_bucket_index(&self, hash: u64) -> usize {
        self.calc_level_hash(hash) as usize
    }

    fn insert_key_on_chain(
        &self,
        k: u64,
        r: &Entry<K, V>,
        mut new_entry: ManuallyDrop<Box<Entry<K, V>>>,
        chain_pos: usize,
    ) {
        if &r.key == &new_entry.key {
            println!("Found existing Key");
            // TODO
            return;
        }

        if let mptr::PtrTarget::HashLevel(next_ref_r) = r.other.load(atomic::Ordering::SeqCst) {
            if next_ref_r == self.own as *mut HashLevel<K, V> {
                let cas_ptr = mptr::mark_as_previous(next_ref_r as *const u8) as *mut u8;

                if chain_pos == self.max_chain {
                    let new_hash = HashLevel::new(self.own, self.bits, self.level + 1);
                    let new_hash_ptr = Box::into_raw(new_hash);
                    match r.other.cas_hashlevel(
                        cas_ptr,
                        new_hash_ptr,
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    ) {
                        Ok(_) => {
                            let bucket_index = self.get_bucket_index(k);
                            let bucket = self.buckets.get(bucket_index).expect(
                                "The Bucket should exist, as it there are always enough buckets",
                            );

                            match bucket.load::<K, V>(atomic::Ordering::SeqCst) {
                                mptr::PtrTarget::Entry(entry_ref_ptr) => {
                                    let new_hash = boxed_hashlevel(new_hash_ptr);
                                    let bucket_entry = boxed_entry(entry_ref_ptr);
                                    new_hash.adjust_chain_nodes(bucket_entry);
                                }
                                _ => {
                                    println!("Expected Bucket to point to Entry");
                                    return;
                                }
                            };

                            bucket.store_hashlevel(new_hash_ptr, atomic::Ordering::SeqCst);

                            let new_hash = boxed_hashlevel(new_hash_ptr);

                            let new_entry = ManuallyDrop::into_inner(new_entry);
                            new_hash.insert_key_on_hash(
                                new_entry.hash,
                                new_entry.key,
                                new_entry.value,
                            );

                            return;
                        }
                        Err(_) => {
                            println!("HashLevel CAS failed");
                        }
                    }

                    return;
                } else {
                    let new_entry_ptr = Box::into_raw(ManuallyDrop::into_inner(new_entry));
                    match r.other.cas_entry(
                        cas_ptr,
                        new_entry_ptr,
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    ) {
                        Ok(_) => return,
                        Err(_) => {
                            new_entry = boxed_entry(new_entry_ptr);
                            println!("Didnt work");
                        }
                    };
                }
            }
        }

        match r.other.load(atomic::Ordering::SeqCst) {
            mptr::PtrTarget::Entry(r) => {
                let r = boxed_entry(r);
                self.insert_key_on_chain(k, &r, new_entry, chain_pos + 1);
            }
            mptr::PtrTarget::HashLevel(r) => {
                let mut r = boxed_hashlevel(r);
                while r.previous != self.own {
                    let n_r = r.previous as *mut HashLevel<K, V>;
                    r = boxed_hashlevel(n_r);
                }

                let inner_entry = ManuallyDrop::into_inner(new_entry);
                r.insert_key_on_hash(k, inner_entry.key, inner_entry.value);
            }
        };
    }

    fn insert_key_on_hash(&self, hash: u64, key: K, value: V) {
        let bucket = self.buckets.get(self.get_bucket_index(hash)).expect(
            "The Bucket should always exist as there Hash should never be bigger than 2^bits",
        );

        let own_marked_ptr = mptr::mark_as_previous(self.own as *const u8) as *mut u8;
        let mut new_entry =
            ManuallyDrop::new(Box::new(Entry::new(hash, key, value, own_marked_ptr)));

        let bucket_ptr = bucket.load(atomic::Ordering::SeqCst);
        if let mptr::PtrTarget::HashLevel(bucket_ptr) = bucket_ptr {
            if bucket_ptr == self.own as *mut HashLevel<K, V> {
                let n_ptr = Box::into_raw(ManuallyDrop::into_inner(new_entry));
                let cas_ptr = mptr::mark_as_previous(bucket_ptr as *const u8) as *mut u8;

                match bucket.cas_entry(
                    cas_ptr,
                    n_ptr,
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

        let bucket_ptr = bucket.load(atomic::Ordering::SeqCst);
        match bucket_ptr {
            mptr::PtrTarget::HashLevel(bucket_ptr) => {
                let raw_new_entry = ManuallyDrop::into_inner(new_entry);

                let sub_lvl = boxed_hashlevel(bucket_ptr);
                sub_lvl.insert_key_on_hash(
                    raw_new_entry.hash,
                    raw_new_entry.key,
                    raw_new_entry.value,
                );
            }
            mptr::PtrTarget::Entry(bucket_ptr) => {
                let current_entry = boxed_entry(bucket_ptr);
                self.insert_key_on_chain(hash, &current_entry, new_entry, 1);
            }
        };
    }

    pub fn insert(&self, hash: u64, key: K, value: V) {
        self.insert_key_on_hash(hash, key, value);
    }

    fn get_chain(
        &self,
        hash: u64,
        current_entry: &Entry<K, V>,
        key: &K,
        chain_pos: usize,
    ) -> Option<V> {
        if &current_entry.key == key {
            return Some(current_entry.value.clone());
        }

        let next_ptr = current_entry.other.load(atomic::Ordering::SeqCst);

        match next_ptr {
            mptr::PtrTarget::HashLevel(next_ptr) => {
                if next_ptr == self.own as *mut HashLevel<K, V> {
                    return None;
                }

                // TODO
                println!("Is new List");
                None
            }
            mptr::PtrTarget::Entry(next_ptr) => {
                let next_entry = boxed_entry(next_ptr);
                self.get_chain(hash, &next_entry, key, chain_pos + 1)
            }
        }
    }

    fn get(&self, hash: u64, key: &K) -> Option<V> {
        let bucket_index = self.get_bucket_index(hash);
        let bucket = self.buckets.get(bucket_index).expect(
            "The Bucket should always exist as there Hash should never be bigger than 2^bits",
        );

        let bucket_ptr = bucket.load(atomic::Ordering::SeqCst);
        if let mptr::PtrTarget::HashLevel(bucket_ptr) = bucket_ptr {
            if bucket_ptr == self.own as *mut HashLevel<K, V> {
                return None;
            }
        }

        let bucket_ptr = bucket.load(atomic::Ordering::SeqCst);
        match bucket_ptr {
            mptr::PtrTarget::Entry(bucket_ptr) => {
                let current_entry = boxed_entry(bucket_ptr);
                let result = self.get_chain(hash, &current_entry, key, 1);
                result
            }
            mptr::PtrTarget::HashLevel(bucket_ptr) => {
                let sub_lvl = boxed_hashlevel(bucket_ptr);
                sub_lvl.get(hash, key)
            }
        }
    }
}

impl<K, V> HashLevel<K, V>
where
    K: Eq + Debug,
    V: Clone + Debug,
{
    pub fn get_clone(&self, hash: u64, key: &K) -> Option<V> {
        self.get(hash, key)
    }
}

impl<K, V> Debug for HashLevel<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let padding = String::from_utf8(vec![b' '; self.level + 1]).unwrap();

        write!(f, "{}Own: {:p} \n", padding, self.own)?;
        for bucket in self.buckets.iter() {
            match bucket.load::<K, V>(atomic::Ordering::SeqCst) {
                mptr::PtrTarget::Entry(entry_ptr) => {
                    let entry = boxed_entry(entry_ptr);
                    write!(f, "{}{:?} \n", padding, entry)?;
                }
                mptr::PtrTarget::HashLevel(hashlvl_ptr)
                    if hashlvl_ptr != self.own as *mut HashLevel<K, V> =>
                {
                    let sub_lvl = ManuallyDrop::into_inner(boxed_hashlevel(hashlvl_ptr));
                    write!(f, "{}HashLevel: \n", padding)?;
                    write!(f, "{:?}", sub_lvl)?;
                    std::mem::forget(sub_lvl);
                }
                mptr::PtrTarget::HashLevel(hashlvl_ptr)
                    if hashlvl_ptr == self.own as *mut HashLevel<K, V> =>
                {
                    write!(f, "{}Empty \n", padding)?;
                }
                _ => {}
            };
        }
        Ok(())
    }
}

pub(crate) struct Entry<K, V> {
    hash: u64,
    key: K,
    value: V,
    other: mptr::Atomic,
}

impl<K, V> Entry<K, V> {
    pub fn new(hash: u64, key: K, value: V, next: *mut u8) -> Self {
        Self {
            hash,
            key,
            value,
            other: mptr::Atomic::new(next),
        }
    }
}

impl<K, V> Debug for Entry<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let other_ptr = match self.other.load::<K, V>(atomic::Ordering::SeqCst) {
            mptr::PtrTarget::Entry(p) => p as *const u8,
            mptr::PtrTarget::HashLevel(p) => p as *const u8,
        };

        write!(
            f,
            "Entry ({:?}:{:?}) -> {:p}",
            self.key, self.value, other_ptr
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_level_calc_hash() {
        let hl_0 = HashLevel::new(0 as *const HashLevel<u64, u64>, 4, 0);

        assert_eq!(0x01, hl_0.calc_level_hash(0x1234567890abcdef));

        let hl_1 = HashLevel::new(0 as *const HashLevel<u64, u64>, 4, 1);
        assert_eq!(0x02, hl_1.calc_level_hash(0x1234567890abcdef));
    }

    #[test]
    fn hash_level_insert_get() {
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64>, 4, 0);

        let hash = 13;
        let key = 16;
        let value = 123;
        hl.insert(hash, key, value);

        assert_eq!(Some(value), hl.get_clone(hash, &16));
    }
    #[test]
    fn hash_level_insert_get_collision() {
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64>, 4, 0);

        let hash = 13;
        let key = 16;
        let value = 123;
        hl.insert(hash, key, value);

        hl.insert(hash, 17, 124);

        assert_eq!(Some(124), hl.get_clone(hash, &17));
    }

    #[test]
    fn hash_level_insert_collision_expand() {
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64>, 4, 0);

        hl.insert(0x1234567890abcdef, 13, 123); // First: 0x1 Second: 0x2
        hl.insert(0x1234567890abcdef, 14, 124); // First: 0x1 Second: 0x2
        hl.insert(0x1334567890abcdef, 15, 125); // First: 0x1 Second: 0x3
        hl.insert(0x1334567890abcdef, 16, 126); // First: 0x1 Second: 0x3

        println!("HashLevel: {:?}", hl);

        assert_eq!(Some(123), hl.get_clone(0x1234567890abcdef, &13));
        assert_eq!(Some(124), hl.get_clone(0x1234567890abcdef, &14));
        assert_eq!(Some(125), hl.get_clone(0x1334567890abcdef, &15));
        assert_eq!(Some(126), hl.get_clone(0x1334567890abcdef, &16));
    }
}
