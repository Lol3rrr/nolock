use std::{
    cell::RefCell,
    sync::{atomic, Arc},
};

use nolock::hazard_ptr;

#[test]
fn protect_boxed() {
    struct Element {
        value: u32,
        dropped: Arc<RefCell<bool>>,
    }
    impl Drop for Element {
        fn drop(&mut self) {
            *self.dropped.borrow_mut() = true;
        }
    }

    let dropped_initial_element = Arc::new(RefCell::new(false));
    let initial_ptr = Box::into_raw(Box::new(Element {
        value: 0,
        dropped: dropped_initial_element.clone(),
    }));

    let list_head = atomic::AtomicPtr::new(initial_ptr);

    let initial_guard = hazard_ptr::protect(
        &list_head,
        atomic::Ordering::SeqCst,
        atomic::Ordering::SeqCst,
    );

    let new_ptr = Box::into_raw(Box::new(Element {
        value: 1,
        dropped: Arc::new(RefCell::new(false)),
    }));
    match list_head.compare_exchange(
        initial_ptr,
        new_ptr,
        atomic::Ordering::SeqCst,
        atomic::Ordering::SeqCst,
    ) {
        Ok(_) => {
            hazard_ptr::retire(initial_ptr, |ptr| {
                let boxed = unsafe { Box::from_raw(ptr) };
                drop(boxed);
            });
        }
        Err(_) => {}
    };

    assert_eq!(0, initial_guard.value);

    let new_guard = hazard_ptr::protect(
        &list_head,
        atomic::Ordering::SeqCst,
        atomic::Ordering::SeqCst,
    );

    assert_eq!(1, new_guard.value);

    drop(initial_guard);
    hazard_ptr::reclaim();

    assert_eq!(true, *dropped_initial_element.borrow());
}
