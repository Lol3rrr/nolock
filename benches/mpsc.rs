use criterion::{black_box, Criterion};

pub fn jiffy_enqueue_dequeue(ctx: &mut Criterion) {
    ctx.bench_function("mpsc-jiffy-enqueue-dequeue", |b| {
        let (mut rx, tx) = nolock::queues::mpsc::jiffy::queue::<u64>();

        b.iter(|| {
            let _ = tx.enqueue(black_box(13));
            assert_eq!(Ok(13), rx.try_dequeue());
        });
    });
}

pub fn std_enqueue_dequeue(ctx: &mut Criterion) {
    ctx.bench_function("mpsc-std-enqueue-dequeue", |b| {
        let (tx, rx) = std::sync::mpsc::channel::<u64>();

        b.iter(|| {
            let _ = tx.send(black_box(13));
            assert_eq!(Ok(13), rx.try_recv());
        });
    });
}
