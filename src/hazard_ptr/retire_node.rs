/// The RetireNode stores a single Pointer to retire as well as the function
/// that should be used to retire the given Piece of Data savely
pub struct RetireNode<T> {
    /// The Data-Pointer that should be retired eventually
    pub ptr: *mut T,
    /// The Function used to actually retire the Data
    retire_fn: Box<dyn Fn(*mut T)>,
}

impl<T> RetireNode<T> {
    /// Creates a new RetireNode with the given Data
    pub fn new(ptr: *mut T, func: Box<dyn Fn(*mut T)>) -> Self {
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
