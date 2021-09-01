mod anchor;
use std::sync::atomic;

use anchor::AtomicAnchor;
pub use anchor::{Anchor, AnchorState};

#[derive(Debug)]
pub struct Descriptor {
    anchor: AtomicAnchor,
    super_block: *mut u8,
    block_size: usize,
    max_count: usize,
    size_class: Option<usize>,
}

impl Descriptor {
    pub fn new(
        block_size: usize,
        max_count: usize,
        size_class: Option<usize>,
        super_block: *mut u8,
    ) -> Self {
        Self {
            anchor: AtomicAnchor::new(Anchor::new(max_count as u32)),
            super_block,
            block_size,
            max_count,
            size_class,
        }
    }

    pub fn max_count(&self) -> usize {
        self.max_count
    }
    pub fn block_size(&self) -> usize {
        self.block_size
    }
    pub fn size_class(&self) -> Option<usize> {
        self.size_class
    }
    pub fn superblock_ptr(&self) -> *mut u8 {
        self.super_block
    }
    pub fn anchor(&self) -> Anchor {
        self.anchor.load(atomic::Ordering::Acquire)
    }

    pub fn contains(&self, ptr: *mut u8) -> bool {
        let ptr_value = ptr as usize;
        let lower_bound = self.super_block as usize;
        let upper_bound = lower_bound + self.block_size * self.max_count;

        lower_bound <= ptr_value && ptr_value <= upper_bound
    }

    pub fn calc_index(&self, ptr: *mut u8) -> u32 {
        let starting_point = self.super_block as usize;
        let offset = (ptr as usize) - starting_point;

        (offset / self.block_size) as u32
    }

    pub fn update_anchor(
        &self,
        current: Anchor,
        new: Anchor,
        success: atomic::Ordering,
        failure: atomic::Ordering,
    ) -> bool {
        self.anchor
            .compare_exchange(current, new, success, failure)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains() {
        let descriptor = Descriptor::new(0x8, 2, Some(0), 0xff as *mut u8);

        assert_eq!(false, descriptor.contains(0xf0 as *mut u8));
        assert_eq!(true, descriptor.contains(0xff as *mut u8));
        assert_eq!(true, descriptor.contains((0xff + 0x8 * 2) as *mut u8));
        assert_eq!(true, descriptor.contains((0xff + 0x8) as *mut u8));
        assert_eq!(false, descriptor.contains((0xff + 0x8 * 3) as *mut u8));
    }

    #[test]
    fn calc_index() {
        let descriptor = Descriptor::new(0x8, 2, Some(0), 0xff as *mut u8);

        assert_eq!(0, descriptor.calc_index(0xff as *mut u8));
        assert_eq!(2, descriptor.calc_index((0xff + 0x8 * 2) as *mut u8));
        assert_eq!(1, descriptor.calc_index((0xff + 0x8) as *mut u8));
    }
}
