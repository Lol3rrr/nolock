use criterion::{criterion_group, criterion_main};

mod hash_trie;
mod mpmc;
mod mpsc;
mod spsc;
mod thread_data;

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
);

criterion_group!(
    thread_data_storage,
    thread_data::storage::list::inserts,
    thread_data::storage::list::gets,
    thread_data::storage::trie::inserts,
    thread_data::storage::trie::gets,
);

criterion_main!(queues, maps, thread_data_storage);
