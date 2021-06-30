use std::{fmt::Debug, mem::ManuallyDrop, sync::atomic};

use crate::{
    hash_trie::{hashlevel::HashLevel, mptr::boxed_entry},
    hazard_ptr,
};

use super::{
    mptr::{self, boxed_hashlevel},
    RefValue,
};

/// This contains all the extra needed "Metadata" for a single Entry in the
/// Map
struct EntryDescription {
    /// This indicates if the current Node is still valid
    valid: atomic::AtomicBool,
}

pub(crate) struct Entry<K, V> {
    pub hash: u64,
    pub key: K,
    pub value: V,
    pub other: mptr::TargetPtr<K, V>,
    description: EntryDescription,
}

impl<K, V> Entry<K, V> {
    pub fn new_hashlevel<const B: u8>(
        hash: u64,
        key: K,
        value: V,
        next: *mut HashLevel<K, V, B>,
    ) -> Box<Self> {
        Box::new(Self {
            hash,
            key,
            value,
            other: mptr::TargetPtr::new_hashlevel(next),
            description: EntryDescription {
                valid: atomic::AtomicBool::new(true),
            },
        })
    }

    pub fn retire(ptr: *mut Self) {
        let boxed = unsafe { Box::from_raw(ptr) };
        drop(boxed);
    }

    pub fn invalidate(&self, order: atomic::Ordering) {
        self.description.valid.store(false, order);
    }
}

#[must_use]
#[derive(Debug)]
pub enum InsertAction {
    ReplaceCurrent,
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
            let entry = ManuallyDrop::into_inner(new_entry);

            // First Remove the record
            h.remove_entry(k, &entry.key);
            // Second Insert again
            h.insert_key_on_hash(k, entry.key, entry.value);
            return;
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
                            let bucket = h.get_bucket(k).expect(
                                "The Bucket should exist, as it there are always enough buckets",
                            );

                            let mut bucket_guard = hazard_ptr::empty_guard();

                            match bucket.load::<B>(&mut bucket_guard) {
                                None => {
                                    let new_hash = boxed_hashlevel(new_hash_ptr);
                                    new_hash.adjust_chain_nodes(bucket_guard);
                                }
                                _ => {
                                    panic!("Expected Bucket to point to an Entry");
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
            Some((mut n_h, _)) => {
                // Find the second level HashLevel
                while n_h.previous != h.own {
                    let n_r = n_h.previous as *mut HashLevel<K, V, B>;
                    n_h = boxed_hashlevel(n_r);
                }

                let inner_entry = ManuallyDrop::into_inner(new_entry);
                n_h.insert_key_on_hash(k, inner_entry.key, inner_entry.value);
            }
        };
    }

    pub fn get_chain<const B: u8>(
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
