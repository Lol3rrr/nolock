use std::alloc::{GlobalAlloc, Layout};

use nolock::allocator::lrmalloc;

#[test]
fn alloc_dealloc() {
    let allocator = lrmalloc::Allocator::new();

    let layout = Layout::new::<usize>();

    for _ in 0..30 {
        let ptr = unsafe { allocator.alloc(layout) };
        unsafe { allocator.dealloc(ptr, layout) };
    }

    let ptr = unsafe { allocator.alloc(layout) };
    unsafe { allocator.dealloc(ptr, layout) };
}
