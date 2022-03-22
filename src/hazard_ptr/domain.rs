mod global;

use crate::sync::atomic;
pub use global::DomainGlobal;
use std::{fmt::Debug, sync::Arc};

use crate::queues::mpsc::jiffy;

use super::{record::Record, retire_node::RetireNode, Guard};

/// A Thread-Local instance to interact with a single Hazard-Pointer-Domain
pub struct TLDomain {
    /// The Refernce to the Shared-Global State for the Hazard-Pointer-Domain
    global: Arc<DomainGlobal>,

    record_sender: Arc<jiffy::Sender<*mut Record<()>>>,
    record_receiver: jiffy::Receiver<*mut Record<()>>,

    /// The Threshold at which it should try to reclaim all Memory marked
    /// as retired
    r_threshold: usize,
    /// The List of Memory-Nodes marked as being ready to retire, by the
    /// algorithm, that have not yet been reclaimed and may still be in use
    /// by some other Part of the overall system
    r_list: Vec<RetireNode>,
}

unsafe impl Send for TLDomain {}

impl Debug for TLDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Domain ()")
    }
}

impl Drop for TLDomain {
    fn drop(&mut self) {
        // This will attempt to retire/reclaim all the waiting entries for the
        // local Thread. Although this is not garantued to free all the marked
        // Nodes, because other Threads might still use the Data, in that case
        // we will leak the memory at the moment
        self.reclaim();

        // TODO
        // Figure out what to do with the remaining Data that could not be
        // retired in this instance
    }
}

impl TLDomain {
    /// Creates a new Domain with the given shared Global and reclaim Threshold
    pub fn new(global: Arc<DomainGlobal>, reclaim_threshold: usize) -> Self {
        let (rx, tx) = jiffy::queue();

        Self {
            r_threshold: reclaim_threshold,
            global,
            record_sender: Arc::new(tx),
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
        // TODO
        // Otherwise we got some Problems in loom which im not really sure about at the moment
        return;
        let plist = self.global.get_protections();

        let tmplist = std::mem::take(&mut self.r_list);

        for node in tmplist {
            if plist.contains(&node.const_ptr()) {
                self.r_list.push(node);
            } else {
                // # Safety
                // This is safe because we have read all the current Hazard-
                // Pointers and no one is protecting this Ptr, meaning that
                // the Data is not accessed by another Thread at the same
                // time and will also not be accessed in the Future and is
                // therefore save to retire.
                unsafe { node.retire() };
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

    /// This function obains an empty Guard, that currently does not protect
    /// anything and should not be used to try and access the Data inside it,
    /// which would cause a Null-Ptr dereference
    pub fn empty_guard<T>(&mut self) -> Guard<T> {
        let record_ptr = match self.record_receiver.try_dequeue() {
            Ok(r) => r,
            _ => self.generate_new_record(),
        };

        Guard::new(std::ptr::null_mut(), record_ptr, self.record_sender.clone())
    }

    /// Loads the most recent Ptr-Value from the given AtomicPtr, protects it
    /// using a Hazard-Ptr and returns a Guard, through which you can access
    /// the underlying protected Data
    pub fn protect<T>(
        &mut self,
        atom_ptr: &atomic::AtomicPtr<T>,
        load_order: atomic::Ordering,
    ) -> Guard<T> {
        let mut guard: Guard<T> = self.empty_guard();

        guard.protect(atom_ptr, load_order);

        guard
    }
}
