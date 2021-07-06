use std::{sync::Arc, thread, time::Duration};

#[cfg(feature = "hash_trie")]
use nolock::hash_trie::HashTrieMap;
use rand::RngCore;

#[cfg(feature = "hash_trie")]
#[test]
fn concurrent_reads_deletes() {
    let map = Arc::new(HashTrieMap::new());

    let max_index = 5000;

    let readers: Vec<_> = (0..10)
        .map(|_| {
            let c_map = map.clone();
            thread::spawn(move || {
                let mut rng = rand::thread_rng();
                for _ in 0..100000 {
                    let k = rng.next_u64() % max_index;
                    c_map.get(&k);
                }
            })
        })
        .collect();

    let updaters: Vec<_> = (0..2)
        .map(|_| {
            let c_map = map.clone();
            thread::spawn(move || {
                let mut rng = rand::thread_rng();
                for _ in 0..1000 {
                    let k = rng.next_u64() % max_index;

                    c_map.remove(&k);
                    thread::sleep(Duration::from_millis(1));
                    c_map.insert(k, 13);
                }
            })
        })
        .collect();

    for th in readers {
        th.join().unwrap();
    }
    for th in updaters {
        th.join().unwrap();
    }
}
