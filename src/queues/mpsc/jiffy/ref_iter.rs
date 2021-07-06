use std::fmt::Debug;

use super::Receiver;

/// Iterator for all Elements from the Queue
///
/// This Iterator behaves nearly identical to the [`OwnedIter`](super::OwnedIter)
/// with the only difference being, that this Iterator does not consume
/// the Queue-Receiver and therefore allows you to use the Receiver
/// for some other checks later on as well
pub struct RefIter<'queue, T> {
    recv: &'queue mut Receiver<T>,
}

impl<'queue, T> RefIter<'queue, T> {
    pub(crate) fn new<'outer_queue>(recv: &'outer_queue mut Receiver<T>) -> Self
    where
        'outer_queue: 'queue,
    {
        Self { recv }
    }
}

impl<'queue, T> Iterator for RefIter<'queue, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv.dequeue()
    }
}

impl<'queue, T> Debug for RefIter<'queue, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ref-Iter ()")
    }
}
