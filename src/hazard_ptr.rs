//! A simple implementation of Hazard-Pointers, that also supports having
//! multiple Hazard-Pointer-Domains
//!
//! # Reference:
//! * [Hazard Pointers: Safe Memory Reclamation for Lock-Free Objects](https://www.eecg.utoronto.ca/~amza/ece1747h/papers/hazard_pointers.pdf)

// TODO
// Add better reuse of Hazard-Pointers, mainly that if a Domain instance is
// dropped, it should mark all of its entries to be reused by other Domains

mod record;
use std::{
    cell::RefCell,
    fmt::Debug,
    sync::{atomic, Arc},
};

use record::Record;

mod retire_node;

mod domain;
use domain::{DomainGlobal, TLDomain};

mod guard;
pub use guard::Guard;

use crate::thread_data::ThreadData;

/// A Hazard-Pointer-Domain that can be used either globally as a shared Domain
/// or as a per Object Domain to seperate the Domains of different Instances of
/// Objects.
///
/// # What is a Hazard-Pointer-Domain
/// A Hazard-Pointer-Domain is a collection of Hazard-Pointers and allows you
/// to seperate different Parts of a System, where you know that they dont need
/// access to the Hazard-Pointers of other Parts.
/// This could lead to performance improvements as all the Parts only check the
/// Hazard-Pointers that are actually relevant for them.
///
/// # Where to use a different Hazard-Pointer-Domain
/// Like previously mentioned different Domains are useful to seperate an
/// entire System into smaller Parts, however there are also other cases where
/// having a custom Domain can be useful.
/// ## Seperating Datastructure Instances
/// Having seperate Domains for individual Datastructures can help with
/// Performance, because for example if you have two Lists and they were to
/// share a single Domain, List 1 has to check the Hazard-Pointers for List 2
/// everytime it needs to work with Hazard-Pointers although they are not
/// relevant in that Case.
#[derive(Clone)]
pub struct Domain {
    global: Arc<DomainGlobal>,
    local: Arc<ThreadData<RefCell<TLDomain>>>,
    reclaim_threshold: usize,
}

impl Debug for Domain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "LocalDomain (reclaim_threshold: {})",
            self.reclaim_threshold
        )
    }
}

impl Domain {
    /// Creates a new Hazard-Pointer-Domain
    ///
    /// # Params
    /// `reclaim_threshold`: The Threshold for waiting Items before attempting
    /// to reclaim Memory
    pub fn new(reclaim_threshold: usize) -> Self {
        Self {
            global: Arc::new(DomainGlobal::new()),
            local: Arc::new(ThreadData::new()),
            reclaim_threshold,
        }
    }

    fn get_local(&self) -> &RefCell<TLDomain> {
        self.local.get_or(|| {
            let global = self.global.clone();
            RefCell::new(TLDomain::new(global, self.reclaim_threshold))
        })
    }

    /// Reads the Data from the given AtomicPtr and protects it using a Hazard-
    /// Ptr.
    /// Returns you a Guard through which you can interact with the Data loaded
    /// from the AtomicPtr and as long as the Guard lives, the Data is safe
    /// to access and use
    ///
    /// # Example
    /// ```rust
    /// # use nolock::hazard_ptr;
    /// # use std::sync::atomic;
    /// let domain = hazard_ptr::Domain::new(10);
    ///
    /// // Create an AtomicPtr with some Value
    /// let ptr = Box::into_raw(Box::new(13));
    /// let atom_ptr = atomic::AtomicPtr::new(ptr);
    ///
    /// // Get protected Access to the Value pointed to
    /// let guarded = domain.protect(&atom_ptr, atomic::Ordering::SeqCst);
    /// // Access the inner Value
    /// assert_eq!(13, *guarded);
    ///
    /// // Retire/"Free" the Data
    /// domain.retire(ptr, |p| { unsafe { Box::from_raw(p) }; });
    ///
    /// // As long as we still have the Guard, the Data will not actually be
    /// // reclaimed and can still be used safely
    /// assert_eq!(13, *guarded);
    /// ```
    pub fn protect<T>(
        &self,
        atom_ptr: &atomic::AtomicPtr<T>,
        load_order: atomic::Ordering,
    ) -> Guard<T> {
        let local = self.get_local();

        let mut shared = local.borrow_mut();
        shared.protect(atom_ptr, load_order)
    }

    /// Creates a new empty Guard, that can then be used to protect any sort of
    /// Data behind an AtomicPtr.
    pub fn empty_guard<T>(&self) -> Guard<T> {
        let local = self.get_local();

        let mut shared = local.borrow_mut();
        shared.empty_guard()
    }

    /// Marks the given Ptr as retired and once no more Hazard-Ptrs protect
    /// the same Ptr, the given `retire_fn` function will be called to
    /// properly clean up the Data.
    ///
    /// # Note
    /// There is no garantue as to when the given Ptr will actually be retired
    /// using the given function, because the Hazard-Pointer that protects the
    /// Data may be stored somewhere or the Thread that was responsible for it
    /// crashed/wont respond/is not running again and therefore can not mark it
    /// as unused anymore.
    pub fn retire<T, F>(&self, ptr: *mut T, retire_fn: F)
    where
        F: Fn(*mut T) + 'static,
    {
        let local = self.get_local();

        let mut shared = local.borrow_mut();
        shared.retire_node(ptr as *mut (), move |raw_ptr| retire_fn(raw_ptr as *mut T));
    }

    /// Forces a reclaimation cycle, however this does not garantue that any
    /// Nodes/Ptrs will actually be reclaimed, as they might all still be
    /// protected/in use
    ///
    /// # Use Cases
    /// This might be useful in a very performance sensitive application, where
    /// you want to avoid running the Reclaimation while in a Hot-Path.
    /// In these Cases, you can set the reclaimation threshold to a very large
    /// Value when creating the Domain, as to avoid triggering it by accident,
    /// and then call this function manually outside of the Hot-Path.
    pub fn reclaim(&self) {
        let local = self.get_local();

        let mut shared = local.borrow_mut();
        shared.reclaim();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic;

    #[derive(Debug, Clone)]
    struct DropCheck {
        d_count: Arc<atomic::AtomicU64>,
    }
    impl DropCheck {
        pub fn new() -> Self {
            Self {
                d_count: Arc::new(atomic::AtomicU64::new(0)),
            }
        }
        pub fn drop_count(&self) -> u64 {
            self.d_count.load(atomic::Ordering::SeqCst)
        }
    }
    impl Drop for DropCheck {
        fn drop(&mut self) {
            self.d_count.fetch_add(1, atomic::Ordering::SeqCst);
        }
    }

    #[test]
    fn local_domain_protect() {
        let drop_chk = DropCheck::new();
        let domain = Arc::new(Domain::new(10));

        let raw_ptr = Box::into_raw(Box::new(drop_chk.clone()));
        let shared_ptr = atomic::AtomicPtr::new(raw_ptr);

        let guard = domain.protect(&shared_ptr, atomic::Ordering::SeqCst);

        assert_eq!(0, guard.drop_count());

        domain.retire(raw_ptr, |ptr| {
            let boxed: Box<DropCheck> = unsafe { Box::from_raw(ptr) };
            drop(boxed);
        });

        domain.reclaim();

        assert_eq!(0, guard.drop_count());

        drop(guard);

        let second_drop_chk = DropCheck::new();
        let other_raw_ptr = Box::into_raw(Box::new(second_drop_chk.clone()));
        shared_ptr.store(other_raw_ptr, atomic::Ordering::SeqCst);

        domain.retire(other_raw_ptr, |ptr| {
            let boxed = unsafe { Box::from_raw(ptr) };
            drop(boxed);
        });

        domain.reclaim();

        assert_eq!(1, drop_chk.drop_count());
        assert_eq!(1, second_drop_chk.drop_count());
    }
}
