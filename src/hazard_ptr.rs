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
pub use domain::{DomainGlobal, TLDomain};

mod guard;
pub use guard::Guard;

/// This macro can be used to generate all the needed parts for a new
/// Hazard-Pointer Domain.
/// This domain will then be available as a private module, with the provided
/// Domain-Name.
///
/// # Domains:
/// A Hazard-Domain helps to seperate different parts of your system that
/// do not share memory and therefore are not relevant, safety wise, for
/// other parts in the System and seperating the Hazard-Pointers by Domain
/// can then help with Performance, as they only need to check the
/// Hazard-Pointers relevant to their Domain.
///
/// # Example:
/// Creates a new Domain called `demo_domain` and then uses it to protect
/// an AtomicPtr and give save access to it;
///
/// ```ignore
/// // Creates a module named `demo_domain` and all the Hazard-Pointer parts
/// // are exposed in that module
/// create_hazard_domain!(demo_domain);
///
/// # use std::sync::atomic;
/// # let boxed_ptr: *mut u8 = Box::into_raw(Box::new(13));
/// # let atomic_ptr = atomic::AtomicPtr::new(boxed_ptr);
///
/// // Actually use the new Hazard-Pointer-Domain
/// let guard = demo_domain::protect(
///     &atomic_ptr,
///     atomic::Ordering::SeqCst,
///     atomic::Ordering::SeqCst
/// );
/// println!("Value in the Guard: {}", *guard);
/// ```
#[macro_export]
macro_rules! create_hazard_domain {
    ($domain_name:ident) => {
        mod $domain_name {
            use crate::hazard_ptr::{TLDomain, DomainGlobal, Guard};
            use std::{
                cell::RefCell,
                sync::{atomic, Arc},
            };

            use lazy_static::lazy_static;

            lazy_static! {
                static ref SUB_GLOBAL: Arc<DomainGlobal> = Arc::new(DomainGlobal::new());
            }

            thread_local! {
                static SUB_DOMAIN: RefCell<TLDomain> = RefCell::new(TLDomain::new(SUB_GLOBAL.clone(), 10));
            }

            /// This functions protects whatever memory address is stored in
            /// the Atomic-Ptr from being freed, while the Guard is still in
            /// use, indicating that the memory is still needed.
            ///
            /// # Behaviour
            /// This function reads the Atomic-Ptr at least 2-times to make
            /// sure that the Ptr was not invalidated before the Hazard has
            /// been updated accordingly.
            ///
            /// # Returns
            /// The Guard returned by this value protects the underlying Memory
            /// as long as it exists and gives you read-only access to
            /// the value stored there
            pub fn protect<T>(
                atom_ptr: &atomic::AtomicPtr<T>,
                load_order: atomic::Ordering,
            ) -> Guard<T> {
                SUB_DOMAIN.with(|shared_domain| {
                    let mut mut_shared = shared_domain.borrow_mut();
                    mut_shared.protect(atom_ptr, load_order)
                })
            }

            /// TODO
            pub fn empty_guard<T>() -> Guard<T> {
                SUB_DOMAIN.with(|shared_domain| {
                    let mut mut_shared = shared_domain.borrow_mut();
                    mut_shared.empty_guard()
                })
            }

            /// This function is used to reclaim a piece of memory, once it is
            /// no longer in use by any other Thread. Once it is determined
            /// that the given Address is no longer used by any other Thread,
            /// the provided `retire_fn` function will be called with the given
            /// Address to then properly reclaim the piece of memory.
            ///
            /// This function does not provide any garantue about when the
            /// memory will be reclaimed, as there is no way to predict when
            /// the memory will not be used anymore
            pub fn retire<T, F>(ptr: *mut T, retire_fn: F)
            where
                F: Fn(*mut T) + 'static,
            {
                SUB_DOMAIN.with(|shared_domain| {
                    let mut mut_shared = shared_domain.borrow_mut();
                    mut_shared
                        .retire_node(ptr as *mut (), move |raw_ptr| retire_fn(raw_ptr as *mut T));
                })
            }

            /// Forces a reclaimation attempt to be performed. However this
            /// does not garantue that any nodes are actually reclaimed as
            /// there might be no unused Node.
            ///
            /// # Usage
            /// This function does not need to be called, as the reclaimation
            /// will be performed automatically once a certain number of items
            /// are waiting to be reclaimed.
            /// However this function might help to improve the Performance of
            /// your Program, as you can call this at a time where you can
            /// spare the Cost of reclaimation without hindering the rest of
            /// the System and therfore help to prevent the reclaimation to
            /// happen in the critical Hot-Path of your Program
            pub fn reclaim() {
                SUB_DOMAIN.with(|shared_domain| {
                    let mut mut_shared = shared_domain.borrow_mut();
                    mut_shared.reclaim();
                });
            }
        }
    };
}

/// TODO
#[derive(Clone)]
pub struct Domain {
    global: Arc<DomainGlobal>,
    local: Arc<thread_local::ThreadLocal<RefCell<TLDomain>>>,
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
    /// Creates a new Domain, which is seperate from other Domains. To get a
    /// Handle to an existing Domain, simply clone the other instance.
    pub fn new() -> Self {
        Self {
            global: Arc::new(DomainGlobal::new()),
            local: Arc::new(thread_local::ThreadLocal::new()),
            reclaim_threshold: 10,
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
    pub fn reclaim(&self) {
        let local = self.get_local();

        let mut shared = local.borrow_mut();
        shared.reclaim();
    }
}

create_hazard_domain!(default);
pub use default::*;

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic;

    #[test]
    fn protect_memory() {
        let raw_ptr = Box::into_raw(Box::new(15));
        let shared_ptr = atomic::AtomicPtr::new(raw_ptr);

        let guard = protect(&shared_ptr, atomic::Ordering::SeqCst);

        assert_eq!(15, *guard);

        retire(raw_ptr, |ptr| {
            let boxed = unsafe { Box::from_raw(ptr) };
            drop(boxed);
        });

        assert_eq!(15, *guard);

        drop(guard);

        let other_raw_ptr = Box::into_raw(Box::new(16));
        shared_ptr.store(other_raw_ptr, atomic::Ordering::SeqCst);

        retire(other_raw_ptr, |ptr| {
            let boxed = unsafe { Box::from_raw(ptr) };
            drop(boxed);
        });
    }

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
        let domain = Arc::new(Domain::new());

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
