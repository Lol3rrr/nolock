mod global;
use std::mem::ManuallyDrop;

pub use global::DomainGlobal;
use std::sync::atomic;

use crate::queues::mpsc::jiffy;

use super::{record::Record, retire_node::RetireNode, Guard};

/// A Hazard-Pointer domain
pub struct Domain {
    reclaim_count: usize,

    global: &'static DomainGlobal,

    record_sender: jiffy::Sender<*mut Record<()>>,
    record_receiver: jiffy::Receiver<*mut Record<()>>,

    // These two are "thread" specific
    r_list: Vec<RetireNode<()>>,
    r_count: usize,
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
    /// Creates a new Domain with the given shared Global and reclaim Count
    pub fn new(global: &'static DomainGlobal, reclaim_count: usize) -> Self {
        let (rx, tx) = jiffy::queue();

        Self {
            reclaim_count,
            global,
            record_sender: tx,
            record_receiver: rx,
            r_list: Vec::new(),
            r_count: 0,
        }
    }

    /// Attempts to retire the Data pointed to by the given Ptr
    pub fn retire_node<F>(&mut self, node: *mut (), retire_func: F)
    where
        F: Fn(*mut ()) + 'static,
    {
        self.r_list
            .push(RetireNode::new(node, Box::new(retire_func)));
        self.r_count += 1;

        // If the number of Backed up retirement Nodes is larger than
        // the specified Boundary, actually retire all the current retirement
        // nodes
        if self.r_count >= self.reclaim_count {
            self.scan();
        }
    }

    /// This actually performs the reclaimation for the elements stored in
    /// the R-List
    fn scan(&mut self) {
        let plist = self.global.get_protections();

        let tmplist = std::mem::replace(&mut self.r_list, Vec::new());
        self.r_count = 0;

        for node in tmplist {
            if plist.contains(&(node.ptr as *mut ())) {
                self.r_list.push(node);
                self.r_count += 1;
            } else {
                node.retire();
            }
        }
    }

    /// Allocates a new Record and appends it to the Global-HP-Records list
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

        let record = ManuallyDrop::new(unsafe { Box::from_raw(record_ptr) });
        let mut protect_ptr = atom_ptr.load(load_order);
        loop {
            record.ptr.store(protect_ptr as *mut (), store_order);

            let n_ptr = atom_ptr.load(load_order);
            if n_ptr == protect_ptr {
                break;
            }

            protect_ptr = n_ptr;
        }

        Guard {
            inner: protect_ptr,
            record: record_ptr,
            record_returner: self.record_sender.clone(),
        }
    }
}
