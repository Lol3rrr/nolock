mod global;

pub use global::DomainGlobal;
use std::sync::atomic;

use crate::queues::mpsc::jiffy;

use super::{record::Record, retire_node::RetireNode, Guard};

/// A Thread-Local instance to interact with a single Hazard-Pointer-Domain
pub struct Domain {
    /// The Refernce to the Shared-Global State for the Hazard-Pointer-Domain
    global: &'static DomainGlobal,

    record_sender: jiffy::Sender<*mut Record<()>>,
    record_receiver: jiffy::Receiver<*mut Record<()>>,

    /// The Threshold at which it should try to reclaim all Memory marked
    /// as retired
    r_threshold: usize,
    /// The List of Memory-Nodes marked as being ready to retire, by the
    /// algorithm, that have not yet been reclaimed and may still be in use
    /// by some other Part of the overall system
    r_list: Vec<RetireNode<()>>,
}

impl Drop for Domain {
    fn drop(&mut self) {
        // TODO
        // This should (at least try to) reclaim all the current
        // Elements in the R-List, as this would otherwise result
        // in a memory leak
    }
}

impl Domain {
    /// Creates a new Domain with the given shared Global and reclaim Threshold
    pub fn new(global: &'static DomainGlobal, reclaim_threshold: usize) -> Self {
        let (rx, tx) = jiffy::queue();

        Self {
            r_threshold: reclaim_threshold,
            global,
            record_sender: tx,
            record_receiver: rx,
            r_list: Vec::new(),
        }
    }

    /// Marks the given Memory-Node as being removed from whatever system it
    /// was part of and that it should be reclaimed at some point, however at
    /// the Moment it might still be used by other Parts. Once the Memory-Node
    /// is no longer used by anyone else, it will reclaim it by calling the
    /// given `retire_func`
    pub fn retire_node<F>(&mut self, node: *mut (), retire_func: F)
    where
        F: Fn(*mut ()) + 'static,
    {
        // Creates a new RetireNode for the given Entry and appends it to
        // the List of Nodes to retire
        let r_node = RetireNode::new(node, Box::new(retire_func));
        self.r_list.push(r_node);

        // If the number of Backed up retirement Nodes is larger than
        // the specified Boundary, actually retire all the current retirement
        // nodes
        if self.r_list.len() >= self.r_threshold {
            self.scan();
        }
    }

    /// Forces a reclaimation attempt, which might reclaim some of the retired
    /// Nodes, but does not garantue that any Node will be reclaimed, as
    /// they might still be used
    pub fn reclaim(&mut self) {
        self.scan();
    }

    /// Actually attempts to reclaim the Memory from the RetireNodes stored
    /// in the Retired-List
    fn scan(&mut self) {
        let plist = self.global.get_protections();

        let tmplist = std::mem::take(&mut self.r_list);

        for node in tmplist {
            if plist.contains(&(node.ptr as *const ())) {
                self.r_list.push(node);
            } else {
                node.retire();
            }
        }
    }

    /// Allocates a new Hazard-Pointer-Record and appends it to the
    /// global shared HP-Records list to make sure that every thread can
    /// see the new Hazard-Pointer as well
    fn generate_new_record(&mut self) -> *mut Record<()> {
        let n_record = Record::boxed_empty();
        let n_record_ptr = Box::into_raw(n_record);

        self.global.append_record(n_record_ptr);

        n_record_ptr
    }

    /// Loads the most recent Ptr-Value from the given AtomicPtr, protects it
    /// using a Hazard-Ptr and returns a Guard, through which you can access
    /// the underlying protected Data
    pub fn protect<T>(
        &mut self,
        atom_ptr: &atomic::AtomicPtr<T>,
        load_order: atomic::Ordering,
        store_order: atomic::Ordering,
    ) -> Guard<T> {
        let record_ptr = match self.record_receiver.dequeue() {
            Some(r) => r,
            None => self.generate_new_record(),
        };

        let mut guard: Guard<T> = Guard {
            inner: std::ptr::null_mut(),
            record: record_ptr,
            record_returner: self.record_sender.clone(),
        };

        guard.protect(atom_ptr, load_order, store_order);

        guard
    }
}
