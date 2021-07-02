use criterion::Criterion;

pub fn spsc_unbounded_queue_inserts(ctx: &mut Criterion) {
    let mut group = ctx.benchmark_group("spsc-unbounded-enqueue-dequeue");

    group.bench_function("buffer-8", |b| {
        let (mut rx, mut tx) = nolock::queues::spsc::unbounded::unbounded_queue(8);
        b.iter(|| {
            tx.enqueue(13);
            rx.try_dequeue();
        });
    });
    group.bench_function("buffer-16", |b| {
        let (mut rx, mut tx) = nolock::queues::spsc::unbounded::unbounded_queue(16);
        b.iter(|| {
            tx.enqueue(13);
            rx.try_dequeue();
        });
    });
    group.bench_function("buffer-32", |b| {
        let (mut rx, mut tx) = nolock::queues::spsc::unbounded::unbounded_queue(32);
        b.iter(|| {
            tx.enqueue(13);
            rx.try_dequeue();
        });
    });
}
