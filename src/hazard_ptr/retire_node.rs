/// The RetireNode stores a single Pointer to retire as well as the function
/// that should be used to retire the given Piece of Data savely
pub struct RetireNode {
    /// The Data-Pointer that should be retired eventually
    pub ptr: *mut (),
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

    /// Actually performs the retirement of the Data
    pub fn retire(self) {
        let retire_fn = self.retire_fn;
        retire_fn(self.ptr);
    }
}
