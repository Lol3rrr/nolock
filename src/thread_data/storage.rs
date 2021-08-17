//! Contains a variety of backing Storage implementations for the Thread-Data
//! Datastructure

mod list;
pub use list::List;

mod trie;
pub use trie::Trie;
