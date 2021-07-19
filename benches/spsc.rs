pub mod crit_bench {
    use criterion::{Criterion, Throughput};

    pub fn spsc_unbounded_queue_inserts(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("spsc-unbounded-queue");

        group.throughput(Throughput::Elements(2));

        group.bench_function("enqueue-dequeue", |b| {
            let (mut rx, mut tx) = nolock::queues::spsc::unbounded::queue();
            b.iter(|| {
                let _ = tx.enqueue(13);
                assert_eq!(Ok(13), rx.try_dequeue());
            });
        });
    }

    pub fn bounded_enqueue_dequeue(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("spsc-bounded-queue");

        group.throughput(Throughput::Elements(2));

        group.bench_function("enqueue-dequeue", |b| {
            let (mut rx, mut tx) = nolock::queues::spsc::bounded::queue(16);
            b.iter(|| {
                assert_eq!(Ok(()), tx.try_enqueue(13));
                assert_eq!(Ok(13), rx.try_dequeue());
            });
        });
    }
}

pub mod iai_bench {
    pub fn unbounded_enqueue_dequeue() {
        let (mut rx, mut tx) = nolock::queues::spsc::unbounded::queue();

        let _ = tx.enqueue(13);
        let _ = rx.dequeue();
    }

    pub fn bounded_enqueue_dequeue() {
        let (mut rx, mut tx) = nolock::queues::spsc::bounded::queue(16);

        let _ = tx.enqueue(13);
        let _ = rx.dequeue();
    }
}
