use core::fmt::Debug;

use super::Receiver;

/// Iterator for all Elements from the Queue
///
/// # Behaviour
/// The Iterator will block on [`next`](Iterator::next) until it either
/// receives an Element from the Queue or when the Queue gets closed by
/// the Producer-Side.
/// This very much behaves like manually calling [`dequeue`](Receiver::dequeue)
/// over and over again until you receive `None`;
///
/// # Example
/// ```
/// # use nolock::queues::mpsc::jiffy;
/// let (rx, tx) = jiffy::queue::<usize>();
///
/// for i in 0..5 {
///   tx.enqueue(i).unwrap();
/// }
/// drop(tx);
///
/// for (element, i) in rx.into_iter().enumerate() {
///   assert_eq!(i, element);
/// }
/// ```
pub struct OwnedIter<T> {
    recv: Receiver<T>,
}

impl<T> OwnedIter<T> {
    /// Creates a new Owned-Iterator for the given Receiver
    pub(crate) fn new(recv: Receiver<T>) -> Self {
        Self { recv }
    }
}

impl<T> Iterator for OwnedIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv.dequeue()
    }
}

impl<T> Debug for OwnedIter<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Owned-Iter ()")
    }
}

#[cfg(test)]
mod tests {
    use crate::queues::mpsc::jiffy;

    use super::*;

    #[test]
    fn iterate() {
        let (rx, tx) = jiffy::queue();

        tx.enqueue(13).unwrap();
        drop(tx);

        let mut rx_iter = OwnedIter::new(rx);

        assert_eq!(Some(13), rx_iter.next());
        assert_eq!(None, rx_iter.next());
    }
}
