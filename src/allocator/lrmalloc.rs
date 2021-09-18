//! A fast lock-free Allocator
//!
//! # Internal design
//! ## Thread-Local Caches
//! Each thread has a small Cache of ready to use allocations, which help with performance
//! in most cases as they dont need any extra synchronization between threads.
//!
//! ## Heap
//! The Heap is the central shared entity, which actually manages the underlying allocations
//! as well as the needed synchronization between different threads.
//!
//! # References
//! * [Paper - 'LRMalloc: a Modern and Competitive Lock-Free Dynamic Memory Allocator'](https://vecpar2018.ncc.unesp.br/wp-content/uploads/2018/09/VECPAR_2018_paper_27.pdf)

use std::{
    alloc::{handle_alloc_error, GlobalAlloc},
    cell::RefCell,
};

mod util;

mod cache;
mod size_classes;
use cache::Cache;
mod heap;
use heap::Heap;
mod pagemap;
use pagemap::PageMap;

mod descriptor;

static PAGEMAP: PageMap = PageMap::new();

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::new());
}

/// The actual Allocator Struct, which can be used for allocating and freeing memory
#[derive(Debug)]
pub struct Allocator {
    heap: Heap,
}

impl Allocator {
    /// Creates a new Instance of the Allocator
    ///
    /// # Note
    /// All Instances of the Allocator share some amount of Data, so they are currently not
    /// independant of each other.
    /// You should only create a single Instance for use as the Global-Allocator of your program
    pub const fn new() -> Self {
        Self { heap: Heap::new() }
    }

    /// Allocates Memory for the given Layout using this allocator
    pub unsafe fn allocate(&self, layout: std::alloc::Layout) -> *mut u8 {
        let size_class = match size_classes::get_size_class_index(layout.size()) {
            Some(s) => s,
            None => {
                return self.heap.allocate_large(layout, &PAGEMAP);
            }
        };

        CACHE.with(|raw| {
            let mut cache = match raw.try_borrow_mut() {
                Ok(r) => r,
                Err(_) => {
                    handle_alloc_error(layout);
                }
            };

            if let Some(ptr) = cache.try_alloc(size_class) {
                return ptr;
            }

            self.heap.fill_cache(&mut cache, size_class, &PAGEMAP);
            cache.try_alloc(size_class).expect("We just filled the Cache with new Blocks, so there should at least be one available block to use for the Allocation")
        })
    }

    /// Deallocates the Memory for the given Ptr with the given Layout
    pub unsafe fn deallocate(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        let desc_ptr = match PAGEMAP.load_descriptor(ptr) {
            Some(ptr) => ptr,
            None => {
                panic!("PTR was not allocated with this allocator");
            }
        };
        let desc = unsafe { &*desc_ptr };

        let size_class = match desc.size_class() {
            Some(s) => s,
            None => {
                self.heap.free_large(ptr, layout, &PAGEMAP);
                return;
            }
        };

        CACHE.with(|raw| {
            let mut cache = raw.borrow_mut();

            if cache.add_block(size_class, ptr).is_err() {
                self.heap.flush_cache(&mut cache, size_class, &PAGEMAP);
                cache.add_block(size_class, ptr).unwrap();
            };
        });
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        unsafe { self.allocate(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        unsafe { self.deallocate(ptr, layout) }
    }
}
