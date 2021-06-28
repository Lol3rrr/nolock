//! A simple implementation of Hazard-Pointers, that also supports having
//! multiple Hazard-Pointer-Domains
//!
//! # Reference:
//! * [Hazard Pointers: Safe Memory Reclamation for Lock-Free Objects](https://www.eecg.utoronto.ca/~amza/ece1747h/papers/hazard_pointers.pdf)

// TODO
// Add better reuse of Hazard-Pointers, mainly that if a Domain instance is
// dropped, it should mark all of its entries to be reused by other Domains

mod record;
use record::Record;

mod retire_node;

mod domain;
pub use domain::{Domain, DomainGlobal};

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
            use crate::hazard_ptr::{Domain, DomainGlobal, Guard};
            use std::{cell::RefCell, sync::atomic};

            static SUB_GLOBAL: DomainGlobal = DomainGlobal::new();

            thread_local! {
                static SUB_DOMAIN: RefCell<Domain> = RefCell::new(Domain::new(&SUB_GLOBAL, 10));
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
                store_order: atomic::Ordering,
            ) -> Guard<T> {
                SUB_DOMAIN.with(|shared_domain| {
                    let mut mut_shared = shared_domain.borrow_mut();
                    mut_shared.protect(atom_ptr, load_order, store_order)
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

            /// TODO
            pub fn reclaim() {
                SUB_DOMAIN.with(|shared_domain| {
                    let mut mut_shared = shared_domain.borrow_mut();
                    mut_shared.reclaim();
                });
            }
        }
    };
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

        let guard = protect(
            &shared_ptr,
            atomic::Ordering::SeqCst,
            atomic::Ordering::SeqCst,
        );

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
}
