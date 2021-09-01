//! A lock-free Allocator
//!
//! # References
//! * [Paper - 'LRMalloc: a Modern and Competitive Lock-Free Dynamic Memory Allocator'](https://vecpar2018.ncc.unesp.br/wp-content/uploads/2018/09/VECPAR_2018_paper_27.pdf)

use std::{
    alloc::{handle_alloc_error, GlobalAlloc},
    cell::RefCell,
};

use lazy_static::lazy_static;

mod util;

mod cache;
mod size_classes;
use cache::Cache;
mod heap;
use heap::Heap;
mod pagemap;
use pagemap::PageMap;

mod descriptor;

lazy_static! {
    static ref HEAP: Heap = Heap::new();
    static ref PAGEMAP: PageMap = PageMap::new();
}

thread_local! {
    static CACHE: RefCell<Cache> = RefCell::new(Cache::new());
}

/// TODO
#[derive(Debug)]
pub struct Allocator {}

impl Allocator {
    /// TODO
    pub const fn new() -> Self {
        Self {}
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        let size_class = match size_classes::get_size_class_index(layout.size()) {
            Some(s) => s,
            None => {
                return HEAP.allocate_large(layout);
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

            HEAP.fill_cache(&mut cache, size_class);
            cache.try_alloc(size_class).expect("We just filled the Cache with new Blocks, so there should at least be one available block to use for the Allocation")
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        let desc_ptr = PAGEMAP.load_descriptor(ptr);
        let desc = unsafe { &*desc_ptr };

        let size_class = match desc.size_class() {
            Some(s) => s,
            None => {
                HEAP.free_large(ptr, layout);
                return;
            }
        };

        CACHE.with(|raw| {
            let mut cache = raw.borrow_mut();

            if cache.add_block(size_class, ptr).is_err() {
                HEAP.flush_cache(&mut cache, size_class);
                cache.add_block(size_class, ptr).unwrap();
            };
        });
    }
}
