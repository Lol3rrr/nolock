use super::size_classes;

mod stack;
use stack::Stack;

#[derive(Debug)]
pub struct Cache {
    stacks: [Stack<u8, 32>; size_classes::size_class_count()],
}

impl Cache {
    pub fn new() -> Self {
        Self {
            stacks: [Stack::new(); size_classes::size_class_count()],
        }
    }

    pub fn try_alloc(&mut self, size_class: usize) -> Option<*mut u8> {
        let stack = self.stacks.get_mut(size_class).expect("The Cache should have a stack for every used Size-Class and this should therefore never fail");
        stack.try_pop()
    }

    pub fn add_block(&mut self, size_class: usize, block: *mut u8) -> Result<(), *mut u8> {
        let stack = self.stacks.get_mut(size_class).expect("");
        stack.try_push(block)
    }
}
