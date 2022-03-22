//! # Hyaline
//! Hyaline is a lock free memory reclaimation scheme
//!
//! # How it works
//! For a detailed description see [this Paper](https://arxiv.org/pdf/1905.07903v2.pdf) on which
//! this implementation is based.
//! Hyaline is an improved version of Reference Counting based memory reclaimation
//!
//! # How to use
//! You first need a Hyaline instance, which you can create using [Hyaline::new], this should be
//! stored alongside your actual Datastructure.
//! Then everytime you want to perform an operation on your Datastructure you call
//! [Hyaline::enter] and the returned Handle can be used to retire Objects, using [Handle::retire],
//! from your Datastructure once they are not accessible anymore by new Threads, at the same time
//! the Handle acts as a Guard, so it should be kept around for as long as you are accessing the
//! Datastructure.
//!
//! https://github.com/rusnikola/lfsmr

use alloc::boxed::Box;
use atomic::Atomic;

use crate::sync;

mod batchlist;

#[derive(Debug, Clone, Copy)]
struct HeadPtr {
    href: u64,
    hptr: *const Node,
}
impl From<u128> for HeadPtr {
    fn from(raw: u128) -> Self {
        Self {
            href: (raw >> 64) as u64,
            hptr: (raw & 0xffffffffffffffff) as *const Node,
        }
    }
}
impl Into<u128> for HeadPtr {
    fn into(self) -> u128 {
        ((self.href as u128) << 64) | (self.hptr as u128)
    }
}

struct LocalBatch {
    nrefnode: *const Node,
    firstnode: *const Node,
}

struct Node {
    meta: NodeMeta,
    nrefnode: *const Node,
    batch_next: *const Node,
    data: *const (),
}

enum NodeMeta {
    NrefNode { nref: sync::atomic::AtomicI64 },
    Others { next: *const Node },
}

/// TODO
pub struct Hyaline<const K: usize = 4> {
    adjs: i64,
    heads: [Atomic<u128>; K],
    batches: batchlist::BatchList<K>,
    free_fn: fn(*const ()),
}

/// TODO
pub struct Handle<'a> {
    hptr: *const Node,
    adjs: i64,
    heads: &'a [Atomic<u128>],
    batch_handle: batchlist::BatchHandle<'a>,
    free_fn: fn(*const ()),
}

const SINGLE_SLOT: Atomic<u128> = Atomic::new(0);

impl<const K: usize> Hyaline<K> {
    /// TODO
    pub fn new(free_fn: fn(*const ())) -> Self {
        Self {
            adjs: (u64::MAX / K as u64).wrapping_add(1) as i64,
            heads: [SINGLE_SLOT; K],
            batches: batchlist::BatchList::new(),
            free_fn,
        }
    }

    /// TODO
    pub fn enter(&self) -> Handle<'_> {
        // TODO
        let slot = 0;

        let last: HeadPtr = self.heads[slot]
            .fetch_add(
                HeadPtr {
                    hptr: core::ptr::null(),
                    href: 1,
                }
                .into(),
                atomic::Ordering::SeqCst,
            )
            .into();

        Handle {
            hptr: last.hptr,
            adjs: self.adjs,
            heads: &self.heads,
            batch_handle: self.batches.get_batch(),
            free_fn: self.free_fn,
        }
    }
}
impl<const K: usize> Drop for Hyaline<K> {
    fn drop(&mut self) {
        for mut handle in self.batches.drain() {
            for node in handle.batch_iter() {
                (self.free_fn)(node);
            }
        }
    }
}

impl<'a> Handle<'a> {
    /// Retires the given Ptr, which will be freed, using the provided `free_fn` when the Hyaline
    /// Instance was created, once it is save to do so.
    ///
    /// # Safety
    /// The given Ptr should not be accessible anymore by any new Thread
    pub unsafe fn retire(&mut self, ptr: *const ()) {
        if self.batch_handle.try_retire(ptr).is_ok() {
            return;
        }

        let nrefnode_ptr = Box::into_raw(Box::new(Node {
            nrefnode: core::ptr::null(),
            batch_next: core::ptr::null(),
            meta: NodeMeta::NrefNode {
                nref: sync::atomic::AtomicI64::new(0),
            },
            data: core::ptr::null(),
        }));

        let (head, _) = self.batch_handle.batch_iter().fold(
            (core::ptr::null_mut(), core::ptr::null_mut()),
            |(mut head, tail): (*mut Node, *mut Node), node| {
                let entry = Node {
                    meta: NodeMeta::Others {
                        next: core::ptr::null(),
                    },
                    batch_next: core::ptr::null(),
                    nrefnode: nrefnode_ptr as *const Node,
                    data: node,
                };
                let entry_ptr = Box::into_raw(Box::new(entry));

                if !tail.is_null() {
                    let tail_node = unsafe { &mut *tail };
                    tail_node.batch_next = entry_ptr as *const Node;
                }
                if head.is_null() {
                    head = entry_ptr;
                }

                (head, entry_ptr)
            },
        );

        unsafe { &mut *nrefnode_ptr }.batch_next = head as *const Node;

        let batch = LocalBatch {
            firstnode: head as *const Node,
            nrefnode: nrefnode_ptr as *const Node,
        };
        self.retire_batch(batch);

        self.batch_handle.try_retire(ptr).unwrap();
    }

    fn retire_batch(&self, batch: LocalBatch) {
        let mut do_adj = false;
        let mut empty: i64 = 0;

        let mut curr_node = batch.firstnode;
        unsafe {
            match &(*batch.nrefnode).meta {
                NodeMeta::NrefNode { nref } => nref.store(0, sync::atomic::Ordering::SeqCst),
                _ => unreachable!(),
            };
        }

        'slot: for raw_head in self.heads.iter() {
            let mut head: HeadPtr;
            loop {
                head = raw_head.load(atomic::Ordering::SeqCst).into();

                if head.href == 0 {
                    do_adj = true;
                    empty = empty.wrapping_add(self.adjs);
                    continue 'slot;
                }

                let new = HeadPtr {
                    hptr: curr_node,
                    href: head.href,
                };

                unsafe {
                    (*(new.hptr as *mut Node)).meta = NodeMeta::Others { next: head.hptr };
                }

                if raw_head
                    .compare_exchange(
                        head.into(),
                        new.into(),
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    )
                    .is_ok()
                {
                    break;
                }
            }

            curr_node = unsafe { (*curr_node).batch_next };
            self.adjust(head.hptr, self.adjs.wrapping_add(head.href as i64));
        }

        if do_adj {
            self.adjust(batch.firstnode, empty);
        }
    }

    fn adjust(&self, node: *const Node, val: i64) {
        if node.is_null() {
            return;
        }

        let node_ref = unsafe { &*node };
        let ref_node = unsafe { &*(node_ref.nrefnode) };

        let ref_val = match &ref_node.meta {
            NodeMeta::NrefNode { nref } => nref,
            _ => return,
        };

        if ref_val.fetch_add(val, sync::atomic::Ordering::SeqCst) == val.wrapping_neg() {
            self.free_batch(ref_node.batch_next);
        }
    }

    fn traverse(&self, mut next: *const Node, handle: &Handle<'_>) {
        loop {
            let current = next;
            if current.is_null() {
                break;
            }

            let current_ref = unsafe { &*current };
            next = match current_ref.meta {
                NodeMeta::Others { next } => next,
                _ => unreachable!(),
            };

            let ref_node = unsafe { &*(current_ref.nrefnode) };
            match &ref_node.meta {
                NodeMeta::NrefNode { nref } => {
                    if nref.fetch_add(-1, sync::atomic::Ordering::SeqCst) == 1 {
                        self.free_batch(ref_node.batch_next);
                    }
                }
                _ => unreachable!(),
            };

            if current == handle.hptr {
                break;
            }
        }
    }

    fn free_batch(&self, start: *const Node) {
        if start.is_null() {
            return;
        }

        let ref_node_ptr = unsafe { &*start }.nrefnode;
        let _ = unsafe { Box::from_raw(ref_node_ptr as *mut Node) };

        let mut current = start;
        while !current.is_null() {
            let node = unsafe { &*current };
            let next = node.batch_next;

            (self.free_fn)(node.data);

            let _ = unsafe { Box::from_raw(current as *mut Node) };

            current = next;
        }
    }
}
impl<'b> Drop for Handle<'b> {
    fn drop(&mut self) {
        // TODO
        let slot = 0;

        let mut next = core::ptr::null();
        let mut current: HeadPtr;
        let mut head: HeadPtr;
        loop {
            head = self.heads[slot].load(atomic::Ordering::SeqCst).into();
            current = head;

            if head.hptr != self.hptr {
                next = match unsafe { &*head.hptr }.meta {
                    NodeMeta::Others { next } => next,
                    _ => unreachable!(),
                };
            }

            let new_hptr = if head.href != 1 {
                head.hptr
            } else {
                core::ptr::null()
            };
            let new_href = head.href - 1;
            let new = HeadPtr {
                hptr: new_hptr,
                href: new_href,
            };

            if self.heads[slot]
                .compare_exchange(
                    head.into(),
                    new.into(),
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                )
                .is_ok()
            {
                break;
            }
        }

        if head.href == 1 && !current.hptr.is_null() {
            self.adjust(current.hptr, self.adjs);
        }
        if current.hptr != self.hptr {
            self.traverse(next, self);
        }
    }
}

#[cfg(all(test, not(loom)))]
mod tests {
    extern crate std;

    use alloc::{sync::Arc, vec::Vec};

    use super::*;

    fn box_dealloc_u8(ptr: *const ()) {
        let _ = unsafe { Box::from_raw(ptr as *mut u8) };
    }

    #[test]
    fn two_threads() {
        let instance = Arc::new(Hyaline::<1>::new(box_dealloc_u8));

        let handles: Vec<_> = (0..2)
            .map(|_| {
                let inst = instance.clone();

                std::thread::spawn(move || {
                    for _ in 0..32 {
                        let mut handle = inst.enter();

                        for i in 0u8..4 {
                            unsafe {
                                handle.retire(Box::into_raw(Box::new(i)) as *const ());
                            }
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}
