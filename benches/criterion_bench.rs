use criterion::{criterion_group, criterion_main, Criterion};

mod allocator;
mod hash_trie;
mod mpmc;
mod mpsc;
mod spsc;
mod thread_data;

mod profiler;

criterion_group!(
    maps,
    hash_trie::hash_trie_inserts,
    hash_trie::std_map_inserts
);

criterion_group!(
    queues,
    spsc::crit_bench::spsc_unbounded_queue_inserts,
    spsc::crit_bench::bounded_enqueue_dequeue,
    mpsc::jiffy_enqueue_dequeue,
    mpsc::std_enqueue_dequeue,
    mpsc::jiffy_concurrent_enqueue,
    mpsc::std_concurrent_enqueue,
    mpmc::ncq_enqueue_dequeue,
    mpmc::scq_enqueue_dequeue,
    mpmc::unbounded_enqueue_dequeue,
);

criterion_group!(
    thread_data_storage,
    thread_data::storage::list::inserts,
    thread_data::storage::list::gets,
    thread_data::storage::trie::inserts,
    thread_data::storage::trie::gets,
);

criterion_group! {
    name = allocator;
    config = Criterion::default().with_profiler(profiler::FlamegraphProfiler::new(100));
    targets = allocator::lrmalloc::allocate_deallocate, allocator::lrmalloc::allocate, allocator::lrmalloc::deallocate, allocator::system_alloc::allocate_deallocate, allocator::system_alloc::allocate, allocator::system_alloc::deallocate,
}

criterion_main!(queues, maps, thread_data_storage, allocator);
