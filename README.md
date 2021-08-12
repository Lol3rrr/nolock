[![Docs](https://docs.rs/nolock/badge.svg)](https://docs.rs/nolock/)
[![Crates.io](https://img.shields.io/crates/v/nolock.svg)](https://crates.io/crates/nolock)

# Nolock
A collection of Lock-Free (sometimes also Wait-Free) algorithms and datastructures

## Goal
The Goal of this Crate is to provide a save, easy to use and fast implementation
for a variety of different Lock-Free or Wait-Free datastructures.

## Feature-Flags
Name | Default | Description
--- | --- | ---
queues | true | Enables the different Queues implementation
async | true | Enables async varients of different Datastructures
thread_data | true | Enables the lockfree Thread-Local-Storage
hazard_ptr | true | Enables the Hazard-Pointer implementation
full | true | Enables all Feature-Flags

## Experimental Feature-Flags
These are Features that may still contain bugs or are not complete yet

Name | Default | Description
--- | --- | ---
hash_trie | true | Enables the Hash-Trie-Map implementation
