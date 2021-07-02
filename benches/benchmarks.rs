use criterion::{criterion_group, criterion_main};

mod mpsc;
mod spsc;

criterion_group!(
    benches,
    spsc::spsc_unbounded_queue_inserts,
    mpsc::mpsc_unbounded_queue_inserts
);

criterion_main!(benches);
