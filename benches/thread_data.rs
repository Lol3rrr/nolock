pub mod storage {
    pub mod list {
        use criterion::{BatchSize, Criterion, Throughput};
        use nolock::thread_data::StorageBackend;

        pub fn inserts(ctx: &mut Criterion) {
            let mut group = ctx.benchmark_group("thread_data::storage::list::inserts");

            group.throughput(Throughput::Elements(1));

            group.bench_function("incrementing ids", |b| {
                let ids: Vec<u64> = (0..64).collect();

                b.iter_batched(
                    || {
                        (
                            ids.clone(),
                            nolock::thread_data::storage::List::<usize>::new(),
                        )
                    },
                    |(ids, list)| {
                        for id in ids {
                            list.insert(id, 123usize);
                        }
                    },
                    BatchSize::SmallInput,
                );
            });
        }

        pub fn gets(ctx: &mut Criterion) {
            let mut group = ctx.benchmark_group("thread_data::storage::list::gets-last");

            group.throughput(Throughput::Elements(1));

            group.bench_function("64-entries", |b| {
                let list = nolock::thread_data::storage::List::<usize>::new();
                for id in 0..64 {
                    list.insert(id, 123usize);
                }

                b.iter(|| {
                    assert_eq!(Some(&123), list.get(63));
                });
            });
        }
    }
    pub mod trie {
        use criterion::{BatchSize, Criterion, Throughput};
        use nolock::thread_data::StorageBackend;

        pub fn inserts(ctx: &mut Criterion) {
            let mut group = ctx.benchmark_group("thread_data::storage::trie::inserts");

            group.throughput(Throughput::Elements(1));

            group.bench_function("incrementing ids", |b| {
                let ids: Vec<u64> = (0..64).collect();

                b.iter_batched(
                    || {
                        (
                            ids.clone(),
                            nolock::thread_data::storage::Trie::<usize>::new(),
                        )
                    },
                    |(ids, list)| {
                        for id in ids {
                            list.insert(id, 123usize);
                        }
                    },
                    BatchSize::SmallInput,
                );
            });
        }

        pub fn gets(ctx: &mut Criterion) {
            let mut group = ctx.benchmark_group("thread_data::storage::trie::gets-last");

            group.throughput(Throughput::Elements(1));

            group.bench_function("64-entries", |b| {
                let list = nolock::thread_data::storage::Trie::<usize>::new();
                for id in 0..64 {
                    list.insert(id, 123usize);
                }

                b.iter(|| {
                    assert_eq!(Some(&123), list.get(63));
                });
            });
        }
    }
}
