use std::{ops::RangeInclusive, sync::atomic};

mod anchor;
pub use anchor::Anchor;
use anchor::AtomicAnchor;

mod anchor_state;
pub use anchor_state::AnchorState;

/// A Descriptor stores all the needed information about any single Superblock
#[derive(Debug)]
pub struct Descriptor {
    /// The Anchor to describe the current State of the Superblock
    anchor: AtomicAnchor,
    /// The Pointer to the start of the Superblock
    super_block: *mut u8,
    /// The Size of each block in the Superblock, this is needed to calculate
    /// the Address for any given Block-Index in the Superblock
    block_size: usize,
    /// The Maximum number of blocks contained in the Superblock
    max_count: usize,
    /// The Size-Class of this Superblock, this is only set if the superblock
    /// is allocated for a given SizeClass
    size_class: Option<usize>,
    /// The Range of addresses that belong to this Superblock
    ptr_range: RangeInclusive<usize>,
}

impl Descriptor {
    /// Creates a new Descriptor based of the given Data
    pub fn new(
        block_size: usize,
        max_count: usize,
        size_class: Option<usize>,
        super_block: *mut u8,
    ) -> Self {
        let lower_bound = super_block as usize;
        let upper_bound = lower_bound + (block_size - 1) * max_count;

        Self {
            anchor: AtomicAnchor::new(Anchor::new(max_count as u32)),
            super_block,
            block_size,
            max_count,
            size_class,
            ptr_range: (lower_bound..=upper_bound),
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

    /// Checks if the given Ptr is contained in the Superblock
    pub fn contains(&self, ptr: *mut u8) -> bool {
        let ptr_value = ptr as usize;
        self.ptr_range.contains(&ptr_value)
    }

    /// Calculates the index of the Block that belongs to the given Ptr
    pub fn calc_index(&self, ptr: *mut u8) -> u32 {
        let starting_point = self.super_block as usize;
        let offset = (ptr as usize) - starting_point;

        (offset / self.block_size) as u32
    }

    /// Updates the Anchor with the given new Anchor using an atomic CAS
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
        assert_eq!(true, descriptor.contains((0xff + 0x8 * 1) as *mut u8));
        assert_eq!(false, descriptor.contains((0xff + 0x8 * 2) as *mut u8));
    }

    #[test]
    fn calc_index() {
        let descriptor = Descriptor::new(0x8, 2, Some(0), 0xff as *mut u8);

        assert_eq!(0, descriptor.calc_index(0xff as *mut u8));
        assert_eq!(2, descriptor.calc_index((0xff + 0x8 * 2) as *mut u8));
        assert_eq!(1, descriptor.calc_index((0xff + 0x8) as *mut u8));
    }
}
