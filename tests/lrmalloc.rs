use nolock::allocator::lrmalloc;

#[global_allocator]
static ALLOCATOR: lrmalloc::Allocator = lrmalloc::Allocator::new();

#[test]
fn large_alloc() {
    let test: Box<[u8; 20000]> = Box::new([0; 20000]);

    drop(test);
}
