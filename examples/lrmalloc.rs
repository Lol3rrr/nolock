use nolock::allocator::lrmalloc;

#[global_allocator]
static ALLOCATOR: lrmalloc::Allocator = lrmalloc::Allocator::new();

fn main() {
    let box_collection: Vec<_> = (0..63).map(|index| Box::new(index)).collect();
    println!("First: {:?}", box_collection);
    drop(box_collection);

    let box_collection: Vec<_> = (0..63).map(|index| Box::new(index)).collect();
    println!("Second: {:?}", box_collection);
    drop(box_collection);

    let test = Box::new(0_u16);

    drop(test);
}
