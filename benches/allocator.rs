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
    b.iter_custom(|iters| {
        let mut result = Duration::ZERO.clone();

        for _ in 0..iters {
            let start = Instant::now();

            let ptr = unsafe { allocator.alloc(layout) };
            unsafe { allocator.dealloc(ptr, layout) };

            result += start.elapsed();
        }

        result
    })
}

fn bench_alloc<A>(b: &mut Bencher, allocator: &A, layout: Layout)
where
    A: GlobalAlloc,
{
    b.iter_custom(|iters| {
        let mut result = Duration::ZERO.clone();

        for _ in 0..iters {
            let start = Instant::now();

            let ptr = unsafe { allocator.alloc(layout) };

            result += start.elapsed();

            unsafe { allocator.dealloc(ptr, layout) };
        }

        result
    })
}

fn bench_dealloc<A>(b: &mut Bencher, allocator: &A, layout: Layout)
where
    A: GlobalAlloc,
{
    b.iter_custom(|iters| {
        let mut result = Duration::ZERO.clone();

        for _ in 0..iters {
            let ptr = unsafe { allocator.alloc(layout) };

            let start = Instant::now();

            unsafe { allocator.dealloc(ptr, layout) };

            result += start.elapsed();
        }

        result
    });
}
