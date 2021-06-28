use std::{fmt::Debug, mem::ManuallyDrop, sync::atomic};

/// A single Record in the List of Hazard-Pointer-Records
pub struct Record<T> {
    /// The underlying Data-Ptr, if the Hazard-Pointer is currently
    /// used, this Ptr is a Null-Ptr
    pub ptr: atomic::AtomicPtr<T>,
    /// The Pointer to the next element in the Linked-List
    pub next: atomic::AtomicPtr<Record<T>>,
}

impl<T> Record<T> {
    /// Creates a new Empty Record, which can be appended to the List
    /// of Records
    pub fn boxed_empty() -> Box<Self> {
        Box::new(Self {
            ptr: atomic::AtomicPtr::new(std::ptr::null_mut()),
            next: atomic::AtomicPtr::new(std::ptr::null_mut()),
        })
    }

    /// Attempts to load the next Element in the Linked-List of Records,
    /// returns None if the Next-Ptr was Null at the Time of reading it,
    /// which might have changed in the mean time
    pub fn load_next(&self, order: atomic::Ordering) -> Option<ManuallyDrop<Box<Self>>> {
        let ptr = self.next.load(order);
        if ptr.is_null() {
            return None;
        }

        Some(ManuallyDrop::new(unsafe { Box::from_raw(ptr) }))
    }

    /// This resets the Hazard-Record to its empty initial State, where it
    /// does not actually protect any Memory and is ready to be acquired and
    /// used
    pub fn reset(&self) {
        self.ptr
            .store(std::ptr::null_mut(), atomic::Ordering::SeqCst);
    }
}

impl<T> Debug for Record<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ptr = self.ptr.load(atomic::Ordering::SeqCst);
        let next = self.next.load(atomic::Ordering::SeqCst);
        write!(f, "Record ( ptr = {:p}, next = {:p} )", ptr, next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_load_next() {
        let record = Record::<u32>::boxed_empty();

        let next_record_ptr = Box::into_raw(Record::boxed_empty());

        assert_eq!(true, record.load_next(atomic::Ordering::SeqCst).is_none());

        record.next.store(next_record_ptr, atomic::Ordering::SeqCst);

        let next_result = record.load_next(atomic::Ordering::SeqCst);
        assert_eq!(true, next_result.is_some());
    }
}
