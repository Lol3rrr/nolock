use std::collections::hash_map::RandomState;

use nolock::hash_trie::HashTrieMap;

fn main() {
    let map: HashTrieMap<String, usize, RandomState> = HashTrieMap::new();

    map.insert("testing".into(), 123);
    map.insert("other".into(), 234);
}
