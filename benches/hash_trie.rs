pub use map::*;

mod map {
    use std::collections::{BTreeMap, HashMap};

    use criterion::Criterion;
    use nolock::hash_trie::HashTrieMap;
    use rand::Rng;

    fn generate_insert_data(size: usize) -> Vec<(u64, u64)> {
        let mut rng = rand::thread_rng();

        let mut result = Vec::new();
        for _ in 0..size {
            let k = rng.gen();
            let v = rng.gen();
            result.push((k, v));
        }
        result
    }

    pub fn hash_trie_inserts(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("hash-trie-batch-inserts");

        for size in [4, 8, 16, 32, 64] {
            group.throughput(criterion::Throughput::Elements(size as u64));

            group.bench_function(size.to_string(), |b| {
                b.iter_batched(
                    || generate_insert_data(size),
                    |data| {
                        let map = HashTrieMap::new();
                        for (k, v) in data {
                            map.insert(k, v);
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            });
        }
    }

    pub fn std_map_inserts(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("hash-trie/stdmap-batch-inserts");

        for size in [4, 8, 16, 32, 64] {
            group.throughput(criterion::Throughput::Elements(size as u64));

            group.bench_function(size.to_string(), |b| {
                b.iter_batched(
                    || generate_insert_data(size),
                    |data| {
                        let mut map = HashMap::new();
                        for (k, v) in data {
                            map.insert(k, v);
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            });
        }
    }

    pub fn std_btree_inserts(ctx: &mut Criterion) {
        let mut group = ctx.benchmark_group("hash-trie/stdbtree-batch-inserts");

        for size in [4, 8, 16, 32, 64] {
            group.throughput(criterion::Throughput::Elements(size as u64));

            group.bench_function(size.to_string(), |b| {
                b.iter_batched(
                    || generate_insert_data(size),
                    |data| {
                        let mut map = BTreeMap::new();
                        for (k, v) in data {
                            map.insert(k, v);
                        }
                    },
                    criterion::BatchSize::SmallInput,
                )
            });
        }
    }
}
