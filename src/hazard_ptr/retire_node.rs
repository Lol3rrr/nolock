/// The RetireNode stores a single Pointer to retire as well as the function
/// that should be used to retire the given Piece of Data savely
pub struct RetireNode {
    /// The Data-Pointer that should be retired eventually
    ptr: *mut (),
    /// The Function used to actually retire the Data
    retire_fn: Box<dyn Fn(*mut ())>,
}

impl RetireNode {
    /// Creates a new RetireNode with the given Data
    pub fn new(ptr: *mut (), func: Box<dyn Fn(*mut ())>) -> Self {
        Self {
            ptr,
            retire_fn: func,
        }
    }

    pub fn const_ptr(&self) -> *const () {
        self.ptr as *const ()
    }

    /// Actually performs the retirement of the Data
    ///
    /// # Safety
    /// The Caller needs to garantue that the Pointer stored in this Node can't
    /// be accessed by anyone else anymore and is currently not used.
    /// This Node needs to have exclusive access to the Data, as the stored
    /// Retire function can freely access the Data and will most likely mutate
    /// or free the underlying Data
    pub unsafe fn retire(self) {
        let retire_fn = self.retire_fn;
        retire_fn(self.ptr);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{atomic, Arc};

    use super::*;

    #[test]
    fn new_node() {
        RetireNode::new(std::ptr::null_mut(), Box::new(|_| {}));
    }

    #[test]
    fn get_ptr() {
        let node = RetireNode::new(0x123 as *mut (), Box::new(|_| {}));
        assert_eq!(0x123 as *const (), node.const_ptr());
    }

    #[test]
    fn retire_node() {
        let retired_ptr = Arc::new(atomic::AtomicPtr::new(std::ptr::null_mut()));

        let node_retire_target = retired_ptr.clone();
        let node = RetireNode::new(
            0x123 as *mut (),
            Box::new(move |ptr| {
                node_retire_target.store(ptr, atomic::Ordering::SeqCst);
            }),
        );

        // Checking that the Value is still null before retiring
        assert_eq!(
            std::ptr::null_mut(),
            retired_ptr.load(atomic::Ordering::SeqCst)
        );

        unsafe { node.retire() };

        assert_eq!(0x123 as *mut (), retired_ptr.load(atomic::Ordering::SeqCst));
    }
}
