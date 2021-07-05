use criterion::{criterion_group, criterion_main};

mod mpsc;
mod spsc;

criterion_group!(
    benches,
    spsc::spsc_unbounded_queue_inserts,
    spsc::bounded_enqueue_dequeue,
    mpsc::jiffy_enqueue_dequeue,
    mpsc::std_enqueue_dequeue,
    mpsc::jiffy_concurrent_enqueue,
    mpsc::std_concurrent_enqueue,
);

criterion_main!(benches);
