use std::{collections::HashSet, fmt::Debug, sync::atomic};

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
    pub fn get_protections(&self) -> HashSet<*const ()> {
        let mut plist = HashSet::new();

        let ptr = self.records.load(atomic::Ordering::SeqCst);
        if ptr.is_null() {
            return plist;
        }

        let mut current = unsafe { &*ptr };

        loop {
            let ptr_val = current.ptr.load(atomic::Ordering::SeqCst);
            if !ptr_val.is_null() {
                plist.insert(ptr_val as *const ());
            }

            let next_ptr = current.next.load(atomic::Ordering::SeqCst);
            if next_ptr.is_null() {
                break;
            }

            current = unsafe { &*next_ptr };
        }

        plist
    }

    /// This is used to add a new Record to the End of the Hazard-Pointer-List
    pub fn append_record(&self, n_record_ptr: *mut Record<()>) {
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
            unsafe { &*ptr }
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

    fn clean_up(&mut self) {
        let current_ptr = self.records.load(atomic::Ordering::SeqCst);
        if current_ptr.is_null() {
            return;
        }

        let mut current = unsafe { Box::from_raw(current_ptr) };
        loop {
            let next_ptr = current.next.load(atomic::Ordering::SeqCst);
            drop(current);

            if next_ptr.is_null() {
                return;
            }
            current = unsafe { Box::from_raw(next_ptr) };
        }
    }
}

impl Drop for DomainGlobal {
    fn drop(&mut self) {
        self.clean_up();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_load_hazards() {
        let global = DomainGlobal::new();

        {
            let expected = HashSet::new();
            assert_eq!(expected, global.get_protections());
        }

        let record_ptr = Box::into_raw(Record::<u64>::boxed_empty());
        global.append_record(record_ptr as *mut Record<()>);

        {
            let expected = HashSet::new();
            assert_eq!(expected, global.get_protections());
        }

        let record = unsafe { &*record_ptr };
        record
            .ptr
            .store(0x123 as *mut u64, atomic::Ordering::SeqCst);

        {
            let mut expected = HashSet::new();
            expected.insert(0x123 as *const ());
            assert_eq!(expected, global.get_protections());
        }

        record
            .ptr
            .store(std::ptr::null_mut(), atomic::Ordering::SeqCst);

        {
            let expected = HashSet::new();
            assert_eq!(expected, global.get_protections());
        }
    }
}
