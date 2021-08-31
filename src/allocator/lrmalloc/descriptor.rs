mod anchor;
use anchor::{Anchor, AnchorState};

#[derive(Debug)]
pub struct Descriptor {
    anchor: Anchor,
    super_block: *mut u8,
    block_size: usize,
    max_count: usize,
    size_class: usize,
}

impl Descriptor {
    pub fn new(
        block_size: usize,
        max_count: usize,
        size_class: usize,
        super_block: *mut u8,
    ) -> Self {
        Self {
            anchor: Anchor::new(max_count as u32),
            super_block,
            block_size,
            max_count,
            size_class,
        }
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }
    pub fn size_class(&self) -> usize {
        self.size_class
    }
    pub fn superblock_ptr(&self) -> *mut u8 {
        self.super_block
    }

    pub fn contains(&self, ptr: *mut u8) -> bool {
        let ptr_value = ptr as usize;
        let lower_bound = self.super_block as usize;
        let upper_bound = lower_bound + self.block_size * self.max_count;

        lower_bound <= ptr_value && ptr_value <= upper_bound
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains() {}
}
