use super::{cache::Cache, descriptor::Descriptor, size_classes, PAGEMAP};

use std::alloc::GlobalAlloc;

mod stack;

pub struct Heap {
    partial: [stack::DescriptorCollection; size_classes::size_class_count()],
}

impl Heap {
    pub fn new() -> Self {
        Self {
            partial: Default::default(),
        }
    }

    pub fn fill_cache(&self, cache: &mut Cache, size_class: usize) {
        if self.fill_cache_from_partial(cache, size_class) {
            return;
        }

        self.fill_cache_from_new(cache, size_class);
    }

    fn fill_cache_from_partial(&self, cache: &mut Cache, size_class: usize) -> bool {
        let partial_col = self.partial.get(size_class).unwrap();
        let desc = match partial_col.try_pop() {
            Some(p) => p,
            None => return false,
        };

        dbg!(desc);

        false
    }

    fn fill_cache_from_new(&self, cache: &mut Cache, size_class: usize) {
        const MAX_COUNT: usize = 32;

        let descriptor_ptr = self.new_superblock::<MAX_COUNT>(size_class);
        let descriptor = unsafe { &*descriptor_ptr };

        for block_index in 0..MAX_COUNT {
            let offset = descriptor.block_size() * block_index;
            let block_ptr = unsafe { descriptor.superblock_ptr().offset(offset as isize) };

            cache.add_block(size_class, block_ptr).expect("");
        }

        PAGEMAP.register_descriptor(descriptor_ptr);
    }

    /// Allocates a new Superblock and creates the corresponding Descriptor
    ///
    /// # Params
    /// * `N`: The Number of blocks in the Superblock
    /// * `size_class`: The Size-Class for the Blocks in the SuperBlock
    fn new_superblock<const N: usize>(&self, size_class: usize) -> *mut Descriptor {
        let block_size = size_classes::get_block_size(size_class);
        let superblock_size = block_size * N;

        let superblock_layout = std::alloc::Layout::from_size_align(superblock_size, 8).unwrap();
        let superblock_ptr = unsafe { std::alloc::System.alloc(superblock_layout) };

        let descriptor = Descriptor::new(block_size, N, size_class, superblock_ptr);
        let descriptor_ptr =
            unsafe { std::alloc::System.alloc(std::alloc::Layout::new::<Descriptor>()) }
                as *mut Descriptor;
        unsafe { descriptor_ptr.write(descriptor) };

        descriptor_ptr
    }
}
