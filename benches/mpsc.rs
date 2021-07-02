use criterion::{black_box, Criterion};

pub fn mpsc_unbounded_queue_inserts(ctx: &mut Criterion) {
    ctx.bench_function("mpsc-jiffy-enqueue-dequeue", |b| {
        let (mut rx, tx) = nolock::queues::mpsc::jiffy::queue::<u64>();

        b.iter(|| {
            tx.enqueue(black_box(13));
            rx.dequeue();
        });
    });
}
