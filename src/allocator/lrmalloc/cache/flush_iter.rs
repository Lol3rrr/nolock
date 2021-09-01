use super::stack::Stack;

pub struct FlushIter<'stack, T, const N: usize> {
    stack: &'stack mut Stack<T, N>,
}

impl<'stack, T, const N: usize> FlushIter<'stack, T, N> {
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
