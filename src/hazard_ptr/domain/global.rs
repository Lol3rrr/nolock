use std::{collections::HashSet, mem::ManuallyDrop, sync::atomic};

use crate::hazard_ptr::Record;

/// TODO
pub struct DomainGlobal {
    records: atomic::AtomicPtr<Record<()>>,
}

impl DomainGlobal {
    /// TODO
    pub const fn new() -> Self {
        let records = atomic::AtomicPtr::new(0 as *mut Record<()>);

        Self { records }
    }

    /// Loads all the currently protected PTRs
    pub fn get_protections(&self) -> HashSet<*mut ()> {
        let mut plist = HashSet::new();

        let ptr = self.records.load(atomic::Ordering::SeqCst);
        if ptr.is_null() {
            return plist;
        }
        let mut current_item = ManuallyDrop::new(unsafe { Box::from_raw(ptr) });

        loop {
            let ptr_val = current_item.ptr.load(atomic::Ordering::SeqCst);
            if !ptr_val.is_null() {
                plist.insert(ptr_val);
            }

            match current_item.load_next(atomic::Ordering::SeqCst) {
                Some(i) => {
                    current_item = i;
                }
                None => break,
            };
        }

        plist
    }

    /// TODO
    pub fn append_record(&self, n_record_ptr: *mut Record<()>) {
        let ptr = self.records.load(atomic::Ordering::SeqCst);
        if ptr.is_null() {
            if let Ok(_) = self.records.compare_exchange(
                0 as *mut Record<()>,
                n_record_ptr,
                atomic::Ordering::SeqCst,
                atomic::Ordering::SeqCst,
            ) {
                return;
            }
        }

        let mut last_record = {
            let ptr = self.records.load(atomic::Ordering::SeqCst);
            ManuallyDrop::new(unsafe { Box::from_raw(ptr) })
        };
        loop {
            match last_record.load_next(atomic::Ordering::SeqCst) {
                Some(l) => {
                    last_record = l;
                }
                None => {
                    match last_record.next.compare_exchange(
                        0 as *mut Record<()>,
                        n_record_ptr,
                        atomic::Ordering::SeqCst,
                        atomic::Ordering::SeqCst,
                    ) {
                        Ok(_) => {
                            break;
                        }
                        Err(_) => {
                            last_record = last_record.load_next(atomic::Ordering::SeqCst).unwrap();
                        }
                    }
                }
            };
        }
    }
}
