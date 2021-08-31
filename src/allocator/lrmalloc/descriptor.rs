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
    pub fn superblock_ptr(&self) -> *mut u8 {
        self.super_block
    }
}
