/// A simple Fixed-Size Stack used for storing the Ptr's for a single SizeClass
/// in the Cache for fast local access
#[derive(Debug, Clone, Copy)]
pub struct Stack<T, const N: usize> {
    /// The Backing storage for the Stack
    buffer: [*mut T; N],
    /// The Number of Entries currently in the Stack
    used: usize,
}

impl<T, const N: usize> Stack<T, N> {
    /// Creates a new empty Stack
    pub const fn new() -> Self {
        Self {
            buffer: [std::ptr::null_mut(); N],
            used: 0,
        }
    }

    /// Attempts to pop a single Element from the Top of the Stack,
    /// returns None if the Stack is empty
    pub fn try_pop(&mut self) -> Option<*mut T> {
        let location = self.used.checked_sub(1)?;

        self.used = location;
        Some(self.buffer[location])
    }

    /// Attempts to push the Ptr onto the Stack,
    /// returns an Error with the given Ptr if the Stack is full
    pub fn try_push(&mut self, ptr: *mut T) -> Result<(), *mut T> {
        if self.used >= N {
            return Err(ptr);
        }

        let location = self.used;
        self.buffer[location] = ptr;
        self.used = location + 1;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {
        Stack::<(), 2>::new();
    }

    #[test]
    fn push_empty() {
        let mut stack: Stack<(), 2> = Stack::new();

        assert_eq!(Ok(()), stack.try_push(0x123 as *mut ()));
    }
    #[test]
    fn push_full() {
        let mut stack: Stack<(), 2> = Stack::new();

        stack.try_push(0x123 as *mut ()).unwrap();
        stack.try_push(0x234 as *mut ()).unwrap();
        assert_eq!(Err(0x345 as *mut ()), stack.try_push(0x345 as *mut ()));
    }

    #[test]
    fn pop_empty() {
        let mut stack: Stack<(), 2> = Stack::new();

        assert_eq!(None, stack.try_pop());
    }
    #[test]
    fn pop_full() {
        let mut stack: Stack<(), 2> = Stack::new();

        stack.try_push(0x123 as *mut ()).unwrap();
        stack.try_push(0x234 as *mut ()).unwrap();
        assert_eq!(Some(0x234 as *mut ()), stack.try_pop());
        assert_eq!(Some(0x123 as *mut ()), stack.try_pop());
    }
}
