use std::{cell::RefCell, sync::atomic};

#[cfg(feature = "hazard_ptr")]
use nolock::hazard_ptr;

#[cfg(feature = "hazard_ptr")]
#[test]
#[cfg(not(loom))]
fn protect_boxed() {
    use std::sync::atomic::AtomicPtr;

    let initial_ptr = Box::into_raw(Box::new(RefCell::new(false)));
    let atom_ptr = AtomicPtr::new(initial_ptr);

    let global = hazard_ptr::get_global_domain();

    let guard = global.protect(&atom_ptr, atomic::Ordering::SeqCst);

    unsafe {
        global.retire(initial_ptr, |ptr| {
            let ref_cell = unsafe { &*ptr };

            *ref_cell.borrow_mut() = true;
        });
    }

    global.reclaim();

    assert_eq!(false, *guard.borrow());

    drop(guard);

    global.reclaim();

    let initial_refcell = unsafe { &*initial_ptr };
    assert_eq!(true, *initial_refcell.borrow());
}
