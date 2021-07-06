use std::{collections::HashSet, fmt::Debug, mem::ManuallyDrop, sync::atomic};

use crate::hazard_ptr::Record;

/// This represents the Global shared state for a singel Hazard-Domain, which
/// is mainly the List of all Hazards in the current Domain
pub struct DomainGlobal {
    records: atomic::AtomicPtr<Record<()>>,
}

impl Debug for DomainGlobal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Domain-Global ()")
    }
}

impl DomainGlobal {
    /// Creates a new Empty DomainGlobal instance, which has no Hazard-Pointers
    /// to start with
    pub const fn new() -> Self {
        let records = atomic::AtomicPtr::new(0 as *mut Record<()>);

        Self { records }
    }

    /// Checks all the current Hazard-Pointers and returns a Set of all
    /// currently protected PTRs stored in them
    pub(crate) fn get_protections(&self) -> HashSet<*const ()> {
        let mut plist = HashSet::new();

        let ptr = self.records.load(atomic::Ordering::SeqCst);
        if ptr.is_null() {
            return plist;
        }
        let mut current_item = ManuallyDrop::new(unsafe { Box::from_raw(ptr) });

        loop {
            let ptr_val = current_item.ptr.load(atomic::Ordering::SeqCst);
            if !ptr_val.is_null() {
                plist.insert(ptr_val as *const ());
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

    /// This is used to add a new Record to the End of the Hazard-Pointer-List
    pub(crate) fn append_record(&self, n_record_ptr: *mut Record<()>) {
        let ptr = self.records.load(atomic::Ordering::SeqCst);
        if ptr.is_null()
            && self
                .records
                .compare_exchange(
                    std::ptr::null_mut(),
                    n_record_ptr,
                    atomic::Ordering::SeqCst,
                    atomic::Ordering::SeqCst,
                )
                .is_ok()
        {
            return;
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
                        std::ptr::null_mut(),
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

impl Drop for DomainGlobal {
    fn drop(&mut self) {
        println!("Dropped Global");
    }
}
