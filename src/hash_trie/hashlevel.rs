use std::{fmt::Debug, marker::PhantomData, mem::ManuallyDrop, sync::atomic};

use crate::hazard_ptr;

use super::{
    mptr::{self, boxed_entry, boxed_hashlevel},
    RefValue,
};

pub(crate) struct HashLevel<K, V, const B: u8> {
    /// The Level of the HashLevel, this is used to determine which bits should
    /// be used to lookup the Key/Hash
    level: usize,
    /// A Ptr to itself
    own: *const HashLevel<K, V, B>,
    /// The Max-Number of Elements that are in a single Chain
    max_chain: usize,
    /// A Ptr to the Previous HashLevel
    previous: *const HashLevel<K, V, B>,
    /// All the buckets for the current one
    buckets: Vec<mptr::TargetPtr<K, V>>,
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
            own: std::ptr::null(),
            buckets,
            _marker: PhantomData,
        });

        let own_ptr = &*result as *const HashLevel<K, V, B>;
        for _ in 0..bucket_count {
            result
                .buckets
                .push(mptr::TargetPtr::new_hashlevel(own_ptr as *mut Self));
        }

        result.own = own_ptr;
        println!("Created new HashLevel: {:p} - {}", own_ptr, result.level);

        result
    }

    /// Filters the given Hash according to the current Hash-Level
    fn calc_level_hash(&self, hash: u64) -> u64 {
        let start = (B as usize) * self.level;
        let end = (B as usize) * (self.level + 1);

        let mask = (u64::MAX << start) >> start;
        (hash & mask) >> (64 - end)
    }

    /// Calculates the Index of the Bucket for a given Hash
    fn get_bucket_index(&self, hash: u64) -> usize {
        self.calc_level_hash(hash) as usize
    }
}

impl<K, V, const B: u8> HashLevel<K, V, B>
where
    K: Eq,
{
    /// Attempts to append the Node `n` to the chain of Node `r`. Additionally
    /// this might cause the allocation of a new HashLevel
    fn adjust_node_on_chain(
        &self,
        n: hazard_ptr::Guard<Entry<K, V>>,
        r: hazard_ptr::Guard<Entry<K, V>>,
        chain: usize,
    ) {
        let mut tmp_guard: hazard_ptr::Guard<Entry<K, V>> = hazard_ptr::empty_guard();

        // Load the Next-Element in the Chain and if it is Hashlevel
        if let Some((_, hash_ptr)) = r.other.load::<B>(&mut tmp_guard) {
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

                        let mut bucket_guard = hazard_ptr::empty_guard();
                        match bucket.load::<B>(&mut bucket_guard) {
                            None => {
                                new_hash.adjust_chain_nodes(bucket_guard);
                            }
                            _ => {
                                println!("Expected Bucket to point to Entry");
                                return;
                            }
                        };

                        bucket.store_hashlevel(new_hash_ptr as *mut (), atomic::Ordering::SeqCst);

                        return;
                    }
                    Err(_) => {
                        println!("Failed CAS");
                    }
                };
            } else {
                // We have reached the End of the Chain, so we should attempt
                // to simply add the new None to the End of the Chain
                let n_ptr = n.raw();
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
        match r.other.load::<B>(&mut tmp_guard) {
            // If the next Element is also an Entry, call this function
            // recursively with the next Entry as the Chain "root"
            None => {
                self.adjust_node_on_chain(n, tmp_guard, chain + 1);
            }
            // If the next Element is a HashLevel, try and insert the node
            // in the next HashLevel after this one
            Some((mut r, _)) => {
                // Go back into the previous HashLevel, until you find the
                // HashLevel, directly "below" the current HashLevel
                while r.previous != self.own {
                    r = boxed_hashlevel(r.previous as *mut Self);
                }

                r.adjust_node_on_hash(n);
            }
        };
    }

    /// Adjusts the Node to fit into the current HashLevel
    fn adjust_node_on_hash(&self, n: hazard_ptr::Guard<Entry<K, V>>) {
        // Set the Next-Element to be the current HashLevel
        n.other
            .store_hashlevel(self.own as *mut (), atomic::Ordering::SeqCst);

        // Find the corresponding Bucket for the given Node
        let bucket_index = self.get_bucket_index(n.hash);
        let bucket = self.buckets.get(bucket_index).unwrap();

        let mut bucket_guard = hazard_ptr::empty_guard();

        // If the Bucket Points to the current HashLevel, the bucket
        // is empty and we can simply CAS the new Node into the Bucket
        if let Some((_, level_ptr)) = bucket.load::<B>(&mut bucket_guard) {
            if level_ptr == self.own as *mut Self {
                let n_ptr = n.raw();

                let marked = mptr::mark_as_previous(level_ptr as *const u8) as *mut u8;
                match bucket.cas_entry::<B>(
                    marked as *mut Entry<K, V>,
                    n_ptr as *mut (),
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => {
                        println!("Insert Worked");
                        return;
                    }
                    Err(_) => {
                        println!("Insert did not work");
                    }
                };
            }
        }

        // Load the bucket Entry again
        match bucket.load::<B>(&mut bucket_guard) {
            // Bucket already contains a Node
            None => {
                println!("Bucket has Entry");
                self.adjust_node_on_chain(n, bucket_guard, 1);
            }
            // Bucket points to a second HashLevel so we should
            // try and adjust the Node "onto" the newly found
            // HashLevel
            Some((r, _)) => {
                println!("Bucket has HashLevel");
                r.adjust_node_on_hash(n);
            }
        };
    }

    /// Starts the adjustment process for the given Node as well as starting
    /// the adjustment for all the Nodes in its Chain
    fn adjust_chain_nodes(&self, r: hazard_ptr::Guard<Entry<K, V>>) {
        let mut tmp_guard = hazard_ptr::empty_guard();
        if let None = r.other.load::<B>(&mut tmp_guard) {
            self.adjust_chain_nodes(tmp_guard);
        }
        self.adjust_node_on_hash(r);
    }

    /// Inserts the new Entry into the current HashLevel
    fn insert_key_on_hash(&self, hash: u64, key: K, value: V) {
        let bucket = self.buckets.get(self.get_bucket_index(hash)).expect(
            "The Bucket should always exist as there Hash should never be bigger than 2^bits",
        );

        let mut new_entry = ManuallyDrop::new(Box::new(Entry::new_hashlevel(
            hash,
            key,
            value,
            self.own as *mut Self,
        )));

        let mut bucket_guard = hazard_ptr::empty_guard();

        // If the
        if let Some((_, bucket_ptr)) = bucket.load(&mut bucket_guard) {
            if bucket_ptr == self.own as *mut Self {
                let n_ptr = Box::into_raw(ManuallyDrop::into_inner(new_entry));
                let cas_ptr = mptr::mark_as_previous(n_ptr as *const u8) as *mut Entry<K, V>;

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

        match bucket.load::<B>(&mut bucket_guard) {
            Some((sub_lvl, sub_lvl_ptr)) => {
                let raw_new_entry = ManuallyDrop::into_inner(new_entry);

                sub_lvl.insert_key_on_hash(
                    raw_new_entry.hash,
                    raw_new_entry.key,
                    raw_new_entry.value,
                );
            }
            None => {
                bucket_guard.insert_key_on_chain(hash, &self, new_entry, 1);
            }
        };
    }

    pub fn insert(&self, hash: u64, key: K, value: V) {
        self.insert_key_on_hash(hash, key, value);
    }

    pub fn get(&self, hash: u64, key: &K) -> Option<RefValue<K, V>> {
        let bucket_index = self.get_bucket_index(hash);
        let bucket = self.buckets.get(bucket_index).expect(
            "The Bucket should always exist as there Hash should never be bigger than 2^bits",
        );

        let mut bucket_guard = hazard_ptr::empty_guard();

        if let Some((_, bucket_ptr)) = bucket.load(&mut bucket_guard) {
            if bucket_ptr == self.own as *mut Self {
                return None;
            }
        }

        match bucket.load::<B>(&mut bucket_guard) {
            None => match bucket_guard.get_chain(hash, &self, key, 1) {
                Ok(v) => Some(v),
                Err(found) if found => Some(RefValue {
                    guard: bucket_guard,
                }),
                _ => None,
            },
            Some((sub_lvl, bucket_ptr)) => sub_lvl.get(hash, key),
        }
    }
}

impl<K, V, const B: u8> Debug for HashLevel<K, V, B>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let padding = String::from_utf8(vec![b' '; self.level + 1]).unwrap();

        writeln!(f, "{}Own: {:p}", padding, self.own)?;
        let mut bucket_guard = hazard_ptr::empty_guard();
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
        }
        Ok(())
    }
}

pub(crate) struct Entry<K, V> {
    hash: u64,
    key: K,
    pub value: V,
    other: mptr::TargetPtr<K, V>,
}

impl<K, V> Entry<K, V> {
    pub fn new_hashlevel<const B: u8>(
        hash: u64,
        key: K,
        value: V,
        next: *mut HashLevel<K, V, B>,
    ) -> Self {
        Self {
            hash,
            key,
            value,
            other: mptr::TargetPtr::new_hashlevel(next),
        }
    }
}

impl<K, V> Entry<K, V>
where
    K: Eq,
{
    /// Appends the `new_entry` onto the current Chain of Entrys
    pub fn insert_key_on_chain<const B: u8>(
        &self,
        k: u64,
        h: &HashLevel<K, V, B>,
        mut new_entry: ManuallyDrop<Box<Self>>,
        chain_pos: usize,
    ) {
        // If the current Node `r` matches given Key, we have found the Target
        // Node/Place
        if self.key == new_entry.key {
            println!("Found existing Key");
            todo!()
            //return;
        }

        let mut other_guard = hazard_ptr::empty_guard();
        match self.other.load(&mut other_guard) {
            // If the next element in the Chain is a HashLevel and points to
            // the current HashLevel, we have reached the end of the Chain
            // and should attempt to insert the Element there
            Some((_, next_ref_r)) if next_ref_r == h.own as *mut HashLevel<K, V, B> => {
                let expected_ptr = mptr::mark_as_previous(h.own as *const u8) as *mut Entry<K, V>;

                // If we reached the Maximum Chain-Length, create a new HashLevel
                // and transfer the Nodes of the current Chain to the new
                // HashLevel
                if chain_pos == h.max_chain {
                    let new_hash = HashLevel::new(h.own, h.level + 1);
                    let new_hash_ptr = Box::into_raw(new_hash);
                    match self.other.cas_hashlevel::<B>(
                        expected_ptr,
                        new_hash_ptr as *mut (),
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    ) {
                        Ok(_) => {
                            let bucket_index = h.get_bucket_index(k);
                            let bucket = h.buckets.get(bucket_index).expect(
                                "The Bucket should exist, as it there are always enough buckets",
                            );

                            let mut bucket_guard = hazard_ptr::empty_guard();

                            match bucket.load::<B>(&mut bucket_guard) {
                                None => {
                                    let new_hash = boxed_hashlevel(new_hash_ptr);
                                    new_hash.adjust_chain_nodes(bucket_guard);
                                }
                                _ => {
                                    println!("Expected Bucket to point to Entry");
                                    return;
                                }
                            };

                            bucket
                                .store_hashlevel(new_hash_ptr as *mut (), atomic::Ordering::SeqCst);

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
                    match self.other.cas_entry::<B>(
                        expected_ptr,
                        new_entry_ptr as *mut (),
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
            _ => {}
        };

        // Load the Next-Element in the Chain
        match self.other.load(&mut other_guard) {
            // If the Next-Element is also an Entry, try to insert the new
            // Element into the Chain
            None => {
                other_guard.insert_key_on_chain(k, h, new_entry, chain_pos + 1);
            }
            // If the Next-Element is a second HashLevel, try and insert
            // the New Node on the Second-Level HashLevel
            Some((mut h, _)) => {
                // Find the second level HashLevel
                while h.previous != h.own {
                    let n_r = h.previous as *mut HashLevel<K, V, B>;
                    h = boxed_hashlevel(n_r);
                }

                let inner_entry = ManuallyDrop::into_inner(new_entry);
                h.insert_key_on_hash(k, inner_entry.key, inner_entry.value);
            }
        };
    }

    fn get_chain<const B: u8>(
        &self,
        hash: u64,
        current_hash: &HashLevel<K, V, B>,
        key: &K,
        chain_pos: usize,
    ) -> Result<RefValue<K, V>, bool> {
        if &self.key == key {
            return Err(true);
        }

        let mut other_guard = hazard_ptr::empty_guard();
        match self.other.load(&mut other_guard) {
            Some((_, next_ptr)) => {
                if next_ptr == current_hash.own as *mut HashLevel<K, V, B> {
                    return Err(false);
                }

                // TODO
                println!("Is new List");
                Err(false)
            }
            None => match other_guard.get_chain(hash, &current_hash, key, chain_pos + 1) {
                Ok(v) => Ok(v),
                Err(found) if found => Ok(RefValue { guard: other_guard }),
                _ => Err(false),
            },
        }
    }
}

impl<K, V> Debug for Entry<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut other_guard = hazard_ptr::empty_guard();
        let other_ptr = match self.other.load::<0>(&mut other_guard) {
            None => other_guard.raw() as *const u8,
            Some((_, p)) => p as *const u8,
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
        let hl_0 = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        assert_eq!(0x01, hl_0.calc_level_hash(0x1234567890abcdef));

        let hl_1 = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 1);
        assert_eq!(0x02, hl_1.calc_level_hash(0x1234567890abcdef));
    }

    #[test]
    #[ignore]
    fn hash_level_insert_get() {
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        let hash = 13;
        let key = 16;
        let value = 123;
        hl.insert(hash, key, value);

        assert_eq!(hl.get(hash, &16).unwrap(), value);
    }
    #[test]
    #[ignore]
    fn hash_level_insert_get_collision() {
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        let hash = 13;
        let key = 16;
        let value = 123;
        hl.insert(hash, key, value);

        hl.insert(hash, 17, 124);

        assert_eq!(hl.get(hash, &17).unwrap(), 124);
    }

    #[test]
    #[ignore]
    fn hash_level_insert_collision_expand() {
        let hl = HashLevel::new(0 as *const HashLevel<u64, u64, 4>, 0);

        hl.insert(0x1234567890abcdef, 13, 123); // First: 0x1 Second: 0x2
        hl.insert(0x1234567890abcdef, 14, 124); // First: 0x1 Second: 0x2
        hl.insert(0x1334567890abcdef, 15, 125); // First: 0x1 Second: 0x3
        hl.insert(0x1334567890abcdef, 16, 126); // First: 0x1 Second: 0x3

        println!("HashLevel: {:?}", hl);

        assert_eq!(hl.get(0x1234567890abcdef, &13).unwrap(), 123);
        assert_eq!(hl.get(0x1234567890abcdef, &14).unwrap(), 124);
        assert_eq!(hl.get(0x1334567890abcdef, &15).unwrap(), 125);
        assert_eq!(hl.get(0x1334567890abcdef, &16).unwrap(), 126);
    }
}
