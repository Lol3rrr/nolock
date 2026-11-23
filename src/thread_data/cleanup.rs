pub struct Cleanup {
    cleanup: Box<dyn FnMut()>,
}

impl Cleanup {
    pub fn new<T, F>(ptr: *mut T, mut cleanup: F) -> Self where F: FnMut(*mut T) + 'static, T: 'static {
        Self {
            cleanup: Box::new(move || {
                cleanup(ptr);
            })
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        (self.cleanup)();
    }
}
