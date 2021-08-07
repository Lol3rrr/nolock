use criterion::{black_box, Criterion, Throughput};

pub fn ncq_enqueue_dequeue(ctx: &mut Criterion) {
    let mut group = ctx.benchmark_group("mpmc-ncq");

    group.throughput(Throughput::Elements(2));

    group.bench_function("enqueue-dequeue", |b| {
        let (rx, tx) = nolock::queues::mpmc::bounded::ncq::queue::<u64>(10);

        b.iter(|| {
            let _ = tx.try_enqueue(black_box(13));
            assert_eq!(Ok(13), rx.try_dequeue());
        });
    });
}

pub fn scq_enqueue_dequeue(ctx: &mut Criterion) {
    let mut group = ctx.benchmark_group("mpmc-scq");

    group.throughput(Throughput::Elements(2));

    group.bench_function("enqueue-dequeue", |b| {
        let (rx, tx) = nolock::queues::mpmc::bounded::scq::queue::<u64>(10);

        b.iter(|| {
            let _ = tx.try_enqueue(black_box(13));
            assert_eq!(Ok(13), rx.try_dequeue());
        });
    });
}
