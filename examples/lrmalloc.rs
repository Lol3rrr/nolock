use nolock::allocator::lrmalloc;

#[global_allocator]
static ALLOCATOR: lrmalloc::Allocator = lrmalloc::Allocator::new();

fn main() {
    let test = Box::new(0_u16);

    drop(test);
}
