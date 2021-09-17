use super::stack::Stack;

/// A FlushIter allows you to iterate over all the Elements in a Stack by popping
/// them from the Top until there are no more Elements left on the Stack. This
/// is mostly used to Flush a given Cache when it is full and needs to be
/// emptied.
pub struct FlushIter<'stack, T, const N: usize> {
    stack: &'stack mut Stack<T, N>,
}

impl<'stack, T, const N: usize> FlushIter<'stack, T, N> {
    /// Creates a new FlushIter for the given Stack
    pub fn new(stack: &'stack mut Stack<T, N>) -> Self {
        Self { stack }
    }
}

impl<'stack, T, const N: usize> Iterator for FlushIter<'stack, T, N> {
    type Item = *mut T;

    fn next(&mut self) -> Option<Self::Item> {
        self.stack.try_pop()
    }
}

#[cfg(test)]
mod tests {
    use crate::allocator::lrmalloc::cache::stack::Stack;

    use super::*;

    #[test]
    fn iter() {
        let mut stack: Stack<u8, 32> = Stack::new();

        stack.try_push(0x123 as *mut u8).unwrap();
        stack.try_push(0x234 as *mut u8).unwrap();

        let mut iter = FlushIter::new(&mut stack);

        assert_eq!(Some(0x234 as *mut u8), iter.next());
        assert_eq!(Some(0x123 as *mut u8), iter.next());
        assert_eq!(None, iter.next());
    }
}
