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

pub fn bounded_enqueue_dequeue(ctx: &mut Criterion) {
    ctx.bench_function("spsc-bounded-enqueue-dequeue", |b| {
        let (mut rx, mut tx) = nolock::queues::spsc::bounded::bounded_queue(16);
        b.iter(|| {
            assert_eq!(Ok(()), tx.try_enqueue(13));
            assert_eq!(Ok(13), rx.try_dequeue());
        });
    });
}
