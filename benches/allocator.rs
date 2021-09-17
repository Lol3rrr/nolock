use std::{
    alloc::{GlobalAlloc, Layout},
    time::{Duration, Instant},
};

use criterion::{black_box, Bencher};

pub mod lrmalloc {
    use criterion::{Criterion, Throughput};
    use nolock::allocator;

    use super::{bench_alloc, bench_alloc_dealloc, bench_dealloc};

    pub fn allocate_deallocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::lrmalloc::alloc_dealloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("alloc-dealloc 1", |b| {
            let allocator = allocator::lrmalloc::Allocator::new();

            let layout = std::alloc::Layout::new::<usize>();

            bench_alloc_dealloc(b, &allocator, layout);
        });
    }

    pub fn allocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::lrmalloc::alloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("alloc-1", |b| {
            let allocator = allocator::lrmalloc::Allocator::new();

            let layout = std::alloc::Layout::new::<usize>();

            bench_alloc(b, &allocator, layout);
        });
    }

    pub fn deallocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::lrmalloc::dealloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("dealloc-1", |b| {
            let allocator = allocator::lrmalloc::Allocator::new();

            let layout = std::alloc::Layout::new::<usize>();

            bench_dealloc(b, &allocator, layout);
        });
    }
}

pub mod system_alloc {
    use super::{bench_alloc, bench_alloc_dealloc, bench_dealloc};

    use criterion::{Criterion, Throughput};

    pub fn allocate_deallocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::system_alloc::alloc_dealloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("alloc-dealloc 1", |b| {
            let allocator = std::alloc::System;

            let layout = std::alloc::Layout::new::<usize>();

            bench_alloc_dealloc(b, &allocator, layout);
        });
    }

    pub fn allocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::system_alloc::alloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("alloc-1", |b| {
            let allocator = std::alloc::System;

            let layout = std::alloc::Layout::new::<usize>();

            bench_alloc(b, &allocator, layout);
        });
    }

    pub fn deallocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::system_alloc::dealloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("dealloc-1", |b| {
            let allocator = std::alloc::System;

            let layout = std::alloc::Layout::new::<usize>();

            bench_dealloc(b, &allocator, layout);
        });
    }
}

fn bench_alloc_dealloc<A>(b: &mut Bencher, allocator: &A, layout: Layout)
where
    A: GlobalAlloc,
{
    const BATCH_SIZE: usize = 30;

    b.iter_custom(|iters| {
        let mut result = Duration::ZERO.clone();

        for _ in 0..(iters / BATCH_SIZE as u64) {
            let start = Instant::now();

            for _ in 0..BATCH_SIZE {
                let ptr = unsafe { allocator.alloc(layout) };
                unsafe { allocator.dealloc(black_box(ptr), layout) };
            }

            result += start.elapsed();
        }

        result
    })
}

fn bench_alloc<A>(b: &mut Bencher, allocator: &A, layout: Layout)
where
    A: GlobalAlloc,
{
    const BATCH_SIZE: usize = 30;

    b.iter_custom(|iters| {
        let mut result = Duration::ZERO.clone();

        let mut tmp_buffer: [*mut u8; BATCH_SIZE] = [std::ptr::null_mut(); BATCH_SIZE];

        for _ in 0..(iters / BATCH_SIZE as u64) {
            let start = Instant::now();

            for i in 0..BATCH_SIZE {
                tmp_buffer[i] = unsafe { allocator.alloc(layout) };
            }

            result += start.elapsed();

            for ptr in tmp_buffer.iter() {
                unsafe { allocator.dealloc(*ptr, layout) };
            }
        }

        result
    })
}

fn bench_dealloc<A>(b: &mut Bencher, allocator: &A, layout: Layout)
where
    A: GlobalAlloc,
{
    const BATCH_SIZE: usize = 30;

    b.iter_custom(|iters| {
        let mut result = Duration::ZERO.clone();

        let mut tmp_buffer: [*mut u8; BATCH_SIZE] = [std::ptr::null_mut(); BATCH_SIZE];

        for _ in 0..(iters / BATCH_SIZE as u64) {
            for i in 0..BATCH_SIZE {
                tmp_buffer[i] = unsafe { allocator.alloc(layout) };
            }

            let start = Instant::now();

            for i in 0..BATCH_SIZE {
                unsafe { allocator.dealloc(tmp_buffer[i], layout) };
            }

            result += start.elapsed();
        }

        result
    });
}
