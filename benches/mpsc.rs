use std::{
    sync::{atomic, Arc},
    thread,
    time::{Duration, Instant},
};

use criterion::{black_box, Criterion, Throughput};

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

pub fn jiffy_concurrent_enqueue(ctx: &mut Criterion) {
    let mut group = ctx.benchmark_group("mpsc-jiffy-conc-enqueue");

    group.throughput(Throughput::Elements(1));

    fn bench_enqueues(iters: u64, thread_count: u64) -> Duration {
        let (rx, tx) = nolock::queues::mpsc::jiffy::queue();
        let a_tx = Arc::new(tx);
        let a_started = Arc::new(atomic::AtomicBool::new(false));

        let per_thread = iters / thread_count;

        let threads: Vec<_> = (0..thread_count)
            .map(|_| {
                let c_tx = a_tx.clone();
                let c_started = a_started.clone();
                thread::spawn(move || {
                    let mut inserted = 0;
                    while !c_started.load(atomic::Ordering::Acquire) {}

                    let started = Instant::now();
                    while inserted < per_thread {
                        c_tx.enqueue(13).unwrap();
                        inserted += 1;
                    }
                    started.elapsed()
                })
            })
            .collect();

        a_started.store(true, atomic::Ordering::Release);

        let mut total_time = Duration::from_nanos(0);
        for th in threads {
            let th_result = th.join().unwrap();
            total_time = total_time + th_result;
        }

        drop(rx);
        drop(a_tx);

        total_time / thread_count as u32
    }

    for threads in [1, 2, 4, 8, 16] {
        group.bench_function(threads.to_string(), |b| {
            b.iter_custom(|iters| bench_enqueues(iters, threads))
        });
    }
}

pub fn std_concurrent_enqueue(ctx: &mut Criterion) {
    let mut group = ctx.benchmark_group("mpsc-std-conc-enqueue");

    group.throughput(Throughput::Elements(1));

    fn bench_enqueues(iters: u64, thread_count: u64) -> Duration {
        let (tx, rx) = std::sync::mpsc::channel();
        let a_started = Arc::new(atomic::AtomicBool::new(false));

        let per_thread = iters / thread_count;

        let threads: Vec<_> = (0..thread_count)
            .map(|_| {
                let c_tx = tx.clone();
                let c_started = a_started.clone();
                thread::spawn(move || {
                    let mut inserted = 0;
                    while !c_started.load(atomic::Ordering::Acquire) {}

                    let started = Instant::now();
                    while inserted < per_thread {
                        c_tx.send(13).unwrap();
                        inserted += 1;
                    }
                    started.elapsed()
                })
            })
            .collect();

        a_started.store(true, atomic::Ordering::Release);

        let mut total_time = Duration::from_nanos(0);
        for th in threads {
            let th_result = th.join().unwrap();
            total_time = total_time + th_result;
        }

        drop(rx);

        total_time / thread_count as u32
    }

    for threads in [1, 2, 4, 8, 16] {
        group.bench_function(threads.to_string(), |b| {
            b.iter_custom(|iters| bench_enqueues(iters, threads))
        });
    }
}
