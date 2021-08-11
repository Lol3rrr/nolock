use std::{
    cell::RefCell,
    sync::{atomic, Arc},
};

#[cfg(feature = "hazard_ptr")]
use nolock::hazard_ptr;

#[cfg(feature = "hazard_ptr")]
#[test]
fn protect_boxed() {}
