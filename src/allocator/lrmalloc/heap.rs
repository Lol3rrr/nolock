use crate::allocator::lrmalloc::descriptor::Anchor;

use super::{
    cache::Cache,
    descriptor::{AnchorState, Descriptor},
    pagemap::PageMap,
    size_classes,
    util::traits::InternalAlloc,
};

use std::{alloc::GlobalAlloc, fmt::Debug, sync::atomic};

mod descriptors;
mod stack;

/// The Heap is responsible for actually managing the Superblocks as well as doing all the needed
/// synchronization between the Threads when needed
pub struct Heap {
    /// This contains a List of parially used Superblocks for every SizeClasses of the Allocator
    partial: [stack::DescriptorCollection; size_classes::size_class_count()],
    /// A Collection of old Descriptors that are ready to be used again for a new Superblock
    recycled_desc: descriptors::RecycleList,
}

impl Debug for Heap {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // TODO
        // Properly implement a Debug implementation
        Ok(())
    }
}

impl Heap {
    /// Creates a new Instance of the Heap
    pub const fn new() -> Self {
        let partial: [stack::DescriptorCollection; size_classes::size_class_count()] = [
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
            stack::DescriptorCollection::new(),
        ];

        Self {
            partial,
            recycled_desc: descriptors::RecycleList::new(),
        }
    }

    pub fn allocate_large(&self, layout: std::alloc::Layout, pagemap: &PageMap) -> *mut u8 {
        let desc_ptr = self.new_superblock::<_, 1>(layout.size(), None, &std::alloc::System);

        pagemap.register_descriptor(desc_ptr);

        let desc = unsafe { &*desc_ptr };
        desc.superblock_ptr()
    }
    pub fn free_large(&self, ptr: *mut u8, layout: std::alloc::Layout, pagemap: &PageMap) {
        let desc_ptr = pagemap.load_descriptor(ptr).expect("This should exist");
        let desc = unsafe { &*desc_ptr };

        self.free_superblock(layout.size(), 1, desc.superblock_ptr());
        self.retire_descriptor(desc_ptr);
    }

    pub fn flush_cache(&self, cache: &mut Cache, size_class: usize, pagemap: &PageMap) {
        let mut flush_iter = cache.flush(size_class).peekable();

        loop {
            let head = match flush_iter.next() {
                Some(h) => h,
                None => return,
            };
            let mut tail = head;

            let head_desc_ptr = pagemap
                .load_descriptor(head)
                .expect("This should also exist");
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
                pagemap.unregister_descriptor(head_desc_ptr);

                self.free_superblock(
                    head_desc.block_size(),
                    head_desc.max_count(),
                    head_desc.superblock_ptr(),
                );
            }
        }
    }

    pub fn fill_cache(&self, cache: &mut Cache, size_class: usize, pagemap: &PageMap) {
        if self.fill_cache_from_partial(cache, size_class) {
            return;
        }

        self.fill_cache_from_new(cache, size_class, pagemap);
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

    fn fill_cache_from_new(&self, cache: &mut Cache, size_class: usize, pagemap: &PageMap) {
        const MAX_COUNT: usize = Cache::get_stack_size();

        let block_size = size_classes::get_block_size(size_class);

        let descriptor_ptr =
            self.new_superblock::<_, MAX_COUNT>(block_size, Some(size_class), &std::alloc::System);
        let descriptor = unsafe { &*descriptor_ptr };

        for block_index in 0..MAX_COUNT {
            let offset = descriptor.block_size() * block_index;
            let block_ptr = unsafe { descriptor.superblock_ptr().add(offset) };

            cache.add_block(size_class, block_ptr).expect("");
        }

        pagemap.register_descriptor(descriptor_ptr);
    }

    /// Allocates a new Superblock and creates the corresponding Descriptor
    ///
    /// # Params
    /// * `N`: The Number of blocks in the Superblock
    /// * `block_size`: The Size of each block in the SuperBlock
    /// * `size_class`: The Size-Class for the Blocks in the SuperBlock
    fn new_superblock<A, const N: usize>(
        &self,
        block_size: usize,
        size_class: Option<usize>,
        allocator: &A,
    ) -> *mut Descriptor
    where
        A: InternalAlloc,
    {
        let superblock_size = block_size * N;

        let superblock_layout = std::alloc::Layout::from_size_align(superblock_size, 8).unwrap();
        let superblock_ptr = allocator.allocate(superblock_layout);

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
    // Right now we are using the system-allocator for all new descriptors,
    // we might switch to using a simple bump allocator for all the descriptors
    fn alloc_descriptor(&self) -> *mut Descriptor {
        if let Some(ptr) = self.recycled_desc.get_descriptor() {
            return ptr;
        }

        let layout = std::alloc::Layout::new::<Descriptor>();
        let raw_ptr = unsafe { std::alloc::System.alloc(layout) };
        raw_ptr as *mut Descriptor
    }
    fn retire_descriptor(&self, desc: *mut Descriptor) {
        self.recycled_desc.add_descriptor(desc);
    }
}

impl Drop for Heap {
    fn drop(&mut self) {
        // TODO
        // Implement Drop
    }
}
