[![Docs](https://docs.rs/nolock/badge.svg)](https://docs.rs/nolock/)
[![Crates.io](https://img.shields.io/crates/v/nolock.svg)](https://crates.io/crates/nolock)

# Nolock
A collection of Lock-Free (sometimes also Wait-Free) algorithms and datastructures

## Goal
The Goal of this Crate is to provide a save, easy to use and fast implementation
for a variety of different Lock-Free or Wait-Free datastructures.

## no_std Support
Rust's no_std is supported for parts of this crate, to use this you need to disable the default
features of the Crate and can then enable the specific features you need. Currently supported are:
* queues

## Feature-Flags
Name | Default | Description
--- | --- | ---
std | true | Enables the std, which is needed for most of the other Features
queues | true | Enables the different Queues implementation
hash_trie | true | Enables the Hash-Trie-Map implementation
async | true | Enables async varients of different Datastructures
thread_data | true | Enables the lockfree Thread-Local-Storage
hazard_ptr | true | Enables the Hazard-Pointer implementation
hyaline | true | Enables the Hyaline implementation
full | true | Enables all Feature-Flags

## Development
### Benchmarking
* Running benchmarks using `cargo bench --bench criterion_bench --`
* Running benchmarks with profiling using `cargo bench --bench criterion_bench -- --profile-time=5`
