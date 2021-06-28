use std::mem::ManuallyDrop;

use std::ops::Deref;
use std::sync::atomic;

use crate::queues::mpsc::jiffy;

use super::record::Record;

/// A Guard protects a single Memory address and provides secure access to
/// it, as long as the Guard is not dropped
pub struct Guard<T> {
    /// The actual Data-Ptr protected by the Hazard-Ptr
    pub(crate) inner: *mut T,
    /// A Ptr to the actual Hazard-Record that protects the underlying Data
    pub(crate) record: *mut Record<()>,
    /// The Queue-Sender on which to return the Hazard-Record once the Guard
    /// is dropped to have a simpler schema for reusing Hazard-Pointers locally
    pub(crate) record_returner: jiffy::Sender<*mut Record<()>>,
}

impl<T> Drop for Guard<T> {
    fn drop(&mut self) {
        let record = ManuallyDrop::new(unsafe { Box::from_raw(self.record) });
        record.reset();

        self.record_returner.enqueue(self.record);
    }
}

impl<T> Deref for Guard<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // # Safety:
        //
        // This is safe to do, because the PTR stored in a Guard will always
        // be valid and the Data behind the PTR will also still be valid as
        // the Guard and it's Hazard-Pointer protect it and therefore prevent
        // it from being deallocated/reclaimed, while the Guard still exists
        unsafe { &*self.inner }
    }
}

impl<T> Guard<T> {
    /// Gets the underlying PTR to the Data protected by the Guard
    pub fn raw(&self) -> *const T {
        self.inner as *const T
    }

    /// Loads the most recent Ptr-Value from the given AtomicPtr and updates
    /// the current Guard to now protect this new Ptr.
    ///
    /// # Usage
    /// This should be used when you already have Guard, but no longer need the
    /// original Guard/Value and now need to protect another new Memory-
    /// Location, as this method reuses an already owned Hazard-Pointer/Guard
    /// and therefore does not need to acquire a Hazard-Pointer beforehand.
    ///
    /// This is especially useful when iterating a Datastruture, as you often
    /// only have one Node you are currently processing and then move on
    /// to another one.
    pub fn protect(&mut self, atom_ptr: &atomic::AtomicPtr<T>, load_order: atomic::Ordering) {
        let record = ManuallyDrop::new(unsafe { Box::from_raw(self.record) });
        let mut protect_ptr = atom_ptr.load(load_order);
        loop {
            record
                .ptr
                .store(protect_ptr as *mut (), atomic::Ordering::SeqCst);

            let n_ptr = atom_ptr.load(load_order);
            if n_ptr == protect_ptr {
                break;
            }

            protect_ptr = n_ptr;
        }

        self.inner = protect_ptr;
    }

    /// Converts the Guard into a Guard for a differnt underlying Type
    pub unsafe fn convert<O>(self) -> Guard<O> {
        std::mem::transmute(self)
    }
}
