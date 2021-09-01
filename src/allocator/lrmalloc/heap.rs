use crate::allocator::lrmalloc::descriptor::Anchor;

use super::{
    cache::Cache,
    descriptor::{AnchorState, Descriptor},
    size_classes, PAGEMAP,
};

use std::{alloc::GlobalAlloc, sync::atomic};

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

    pub fn allocate_large(&self, layout: std::alloc::Layout) -> *mut u8 {
        let desc_ptr = self.new_superblock::<1>(layout.size(), None);

        PAGEMAP.register_descriptor(desc_ptr);

        let desc = unsafe { &*desc_ptr };
        desc.superblock_ptr()
    }
    pub fn free_large(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        let desc_ptr = PAGEMAP.load_descriptor(ptr);
        let desc = unsafe { &*desc_ptr };

        self.free_superblock(layout.size(), 1, desc.superblock_ptr());
        self.retire_descriptor(desc_ptr);
    }

    pub fn flush_cache(&self, cache: &mut Cache, size_class: usize) {
        let mut flush_iter = cache.flush(size_class).peekable();

        loop {
            let head = match flush_iter.next() {
                Some(h) => h,
                None => return,
            };
            let mut tail = head;

            let head_desc_ptr = PAGEMAP.load_descriptor(head);
            let head_desc = unsafe { &*head_desc_ptr };
            let mut block_count = 1;

            while let Some(block) = flush_iter.peek() {
                if !head_desc.contains(*block) {
                    break;
                }

                let block = flush_iter.next().expect("We previusly peeked and found an item, so now we should still have an item in the Iterator");
                block_count += 1;
                unsafe { (tail as *mut *mut u8).write(block) };
                tail = block;
            }

            let superblock_ptr = head_desc.superblock_ptr();
            let index = head_desc.calc_index(head);

            let mut old_anchor;
            let mut new_anchor;
            loop {
                old_anchor = head_desc.anchor();
                new_anchor = old_anchor;

                let old_first_ptr = ((superblock_ptr as usize)
                    + old_anchor.avail as usize * head_desc.block_size() as usize)
                    as *mut u8;
                unsafe { (tail as *mut *mut u8).write(old_first_ptr) };

                new_anchor.state = AnchorState::Partial;
                new_anchor.avail = index;
                new_anchor.count += block_count;

                if new_anchor.count == head_desc.max_count() as u32 {
                    new_anchor.state = AnchorState::Empty;
                }

                if head_desc.update_anchor(
                    old_anchor,
                    new_anchor,
                    atomic::Ordering::AcqRel,
                    atomic::Ordering::Relaxed,
                ) {
                    break;
                }
            }

            if let AnchorState::Full = old_anchor.state {
                let partial = self.partial.get(size_class).expect("");
                partial.push(head_desc_ptr);
            } else if let AnchorState::Empty = new_anchor.state {
                PAGEMAP.unregister_descriptor(head_desc_ptr);

                self.free_superblock(
                    head_desc.block_size(),
                    head_desc.max_count(),
                    head_desc.superblock_ptr(),
                );
            }
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
        let desc_ptr = match partial_col.try_pop() {
            Some(p) => p,
            None => return false,
        };
        let desc = unsafe { &*desc_ptr };

        let mut old_anchor;
        let mut new_anchor = Anchor::new(0);
        loop {
            old_anchor = desc.anchor();
            if let AnchorState::Empty = old_anchor.state {
                self.retire_descriptor(desc_ptr);
                return self.fill_cache_from_partial(cache, size_class);
            }

            new_anchor.state = AnchorState::Full;
            new_anchor.avail = desc.max_count() as u32;
            new_anchor.count = 0;

            if desc.update_anchor(
                old_anchor,
                new_anchor,
                atomic::Ordering::AcqRel,
                atomic::Ordering::Relaxed,
            ) {
                break;
            }
        }

        let mut current_ptr = (desc.superblock_ptr() as usize
            + old_anchor.avail as usize * desc.block_size())
            as *mut u8;
        for _ in 0..old_anchor.count {
            cache.add_block(size_class, current_ptr).unwrap();

            current_ptr = unsafe { (current_ptr as *mut *mut u8).read() };
        }

        true
    }

    fn fill_cache_from_new(&self, cache: &mut Cache, size_class: usize) {
        const MAX_COUNT: usize = 32;

        let block_size = size_classes::get_block_size(size_class);

        let descriptor_ptr = self.new_superblock::<MAX_COUNT>(block_size, Some(size_class));
        let descriptor = unsafe { &*descriptor_ptr };

        for block_index in 0..MAX_COUNT {
            let offset = descriptor.block_size() * block_index;
            let block_ptr = unsafe { descriptor.superblock_ptr().add(offset) };

            cache.add_block(size_class, block_ptr).expect("");
        }

        PAGEMAP.register_descriptor(descriptor_ptr);
    }

    /// Allocates a new Superblock and creates the corresponding Descriptor
    ///
    /// # Params
    /// * `N`: The Number of blocks in the Superblock
    /// * `block_size`: The Size of each block in the SuperBlock
    /// * `size_class`: The Size-Class for the Blocks in the SuperBlock
    fn new_superblock<const N: usize>(
        &self,
        block_size: usize,
        size_class: Option<usize>,
    ) -> *mut Descriptor {
        let superblock_size = block_size * N;

        let superblock_layout = std::alloc::Layout::from_size_align(superblock_size, 8).unwrap();
        let superblock_ptr = unsafe { std::alloc::System.alloc(superblock_layout) };

        let descriptor = Descriptor::new(block_size, N, size_class, superblock_ptr);
        let descriptor_ptr = self.alloc_descriptor();
        unsafe { descriptor_ptr.write(descriptor) };

        descriptor_ptr
    }

    fn free_superblock(&self, block_size: usize, block_count: usize, superblock_ptr: *mut u8) {
        let size = block_size * block_count;
        let layout = std::alloc::Layout::from_size_align(size, 8).unwrap();
        unsafe { std::alloc::System.dealloc(superblock_ptr, layout) };
    }

    // TODO
    // Instead of using the System-Allocator for all the Descriptors and then simply leaking them,
    // we should instead keep a collection of retired descriptors and then first attempt to load
    // a free one from that collection
    fn alloc_descriptor(&self) -> *mut Descriptor {
        let layout = std::alloc::Layout::new::<Descriptor>();
        let raw_ptr = unsafe { std::alloc::System.alloc(layout) };
        raw_ptr as *mut Descriptor
    }
    fn retire_descriptor(&self, desc: *mut Descriptor) {
        dbg!(desc);
    }
}
