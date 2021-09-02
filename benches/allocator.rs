use std::alloc::{GlobalAlloc, Layout};

use criterion::black_box;

fn alloc_dealloc<A>(allocator: &A, layout: Layout)
where
    A: GlobalAlloc,
{
    let ptr = unsafe { allocator.alloc(layout) };

    unsafe { allocator.dealloc(black_box(ptr), layout) };
}

pub mod lrmalloc {
    use criterion::{Criterion, Throughput};
    use nolock::allocator;

    use super::alloc_dealloc;

    pub fn allocate_deallocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::lrmalloc::alloc_dealloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("alloc-dealloc 1", |b| {
            let allocator = allocator::lrmalloc::Allocator::new();

            let layout = std::alloc::Layout::new::<usize>();

            b.iter(|| {
                alloc_dealloc(&allocator, layout);
            });
        });
    }
}

pub mod system_alloc {
    use super::alloc_dealloc;

    use criterion::{Criterion, Throughput};

    pub fn allocate_deallocate(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("allocator::system_alloc::alloc_dealloc");

        group.throughput(Throughput::Elements(1));

        group.bench_function("alloc-dealloc 1", |b| {
            let allocator = std::alloc::System;

            let layout = std::alloc::Layout::new::<usize>();

            b.iter(|| {
                alloc_dealloc(&allocator, layout);
            });
        });
    }
}
