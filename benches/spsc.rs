use criterion::Criterion;

pub fn spsc_unbounded_queue_inserts(ctx: &mut Criterion) {
    ctx.bench_function("spsc-unbounded-enqueue-dequeue", |b| {
        let (mut rx, mut tx) = nolock::queues::spsc::unbounded::queue();
        b.iter(|| {
            let _ = tx.enqueue(13);
            assert_eq!(Ok(13), rx.try_dequeue());
        });
    });
}

pub fn bounded_enqueue_dequeue(ctx: &mut Criterion) {
    ctx.bench_function("spsc-bounded-enqueue-dequeue", |b| {
        let (mut rx, mut tx) = nolock::queues::spsc::bounded::queue(16);
        b.iter(|| {
            assert_eq!(Ok(()), tx.try_enqueue(13));
            assert_eq!(Ok(13), rx.try_dequeue());
        });
    });
}
