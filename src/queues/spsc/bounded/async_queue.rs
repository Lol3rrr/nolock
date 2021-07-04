use std::{fmt::Debug, future::Future, sync::Arc, task::Poll};

use futures::task::AtomicWaker;

use crate::queues::spsc::{DequeueError, EnqueueError};

use super::{BoundedReceiver, BoundedSender};

/// An async variant of the [`BoundedSender`] that allows your to efficiently
/// use this in async Contexts as well.
///
/// Created using the [`async_queue`] method
pub struct AsyncBoundedSender<T> {
    rx_waker: Arc<AtomicWaker>,
    tx_waker: Arc<AtomicWaker>,
    queue: BoundedSender<T>,
}

/// An async variant of the [`BoundedReceiver`] that allows your to efficiently
/// use this in async Contexts as well.
///
/// Created using the [`async_queue`] method
pub struct AsyncBoundedReceiver<T> {
    rx_waker: Arc<AtomicWaker>,
    tx_waker: Arc<AtomicWaker>,
    queue: BoundedReceiver<T>,
}

/// The Future returned when attempting to enqueue an Item
pub struct EnqueueFuture<'queue, T> {
    rx_waker: &'queue AtomicWaker,
    tx_waker: &'queue AtomicWaker,
    queue: &'queue mut BoundedSender<T>,
    data: Option<T>,
}

/// The Future returned when attempting to dequeue an Item
pub struct DequeueFuture<'queue, T> {
    rx_waker: &'queue AtomicWaker,
    tx_waker: &'queue AtomicWaker,
    queue: &'queue mut BoundedReceiver<T>,
}

impl<T> AsyncBoundedSender<T> {
    /// The async variant of the blocking [`enqueue`](BoundedSender::enqueue)
    /// operation on the Non-Async version of the Queue
    pub fn enqueue<'queue>(&'queue mut self, data: T) -> EnqueueFuture<'queue, T> {
        EnqueueFuture {
            rx_waker: &self.rx_waker,
            tx_waker: &self.tx_waker,
            queue: &mut self.queue,
            data: Some(data),
        }
    }

    /// Attempts to enqueue the given Data on the Queue.
    ///
    /// This behaves just like the non-async
    /// [`try_enqueue`](BoundedSender::try_enqueue) operation
    pub fn try_enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
        match self.queue.try_enqueue(data) {
            Ok(_) => {
                self.rx_waker.wake();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

impl<T> Debug for AsyncBoundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Bounded-Sender ()")
    }
}

impl<T> AsyncBoundedReceiver<T> {
    /// The async variant of the blocking [`dequeue`](BoundedReceiver::dequeue)
    /// operation on the Non-Async version of the Queue
    pub fn dequeue<'queue>(&'queue mut self) -> DequeueFuture<'queue, T> {
        DequeueFuture {
            rx_waker: &self.rx_waker,
            tx_waker: &self.tx_waker,
            queue: &mut self.queue,
        }
    }

    /// Attempts to dequeue a single Item from the Queue.
    ///
    /// This behaves just like the non-async
    /// [`try_dequeue`](BoundedReceiver::try_dequeue) operation
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        match self.queue.try_dequeue() {
            Ok(d) => {
                self.tx_waker.wake();
                Ok(d)
            }
            Err(e) => Err(e),
        }
    }
}

impl<T> Debug for AsyncBoundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Bounded-Receiver ()")
    }
}

impl<'queue, T> Unpin for EnqueueFuture<'queue, T> {}

impl<'queue, T> Future for EnqueueFuture<'queue, T> {
    type Output = Result<(), (T, EnqueueError)>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let data = match self.data.take() {
            Some(d) => d,
            None => return Poll::Ready(Ok(())),
        };

        match self.queue.try_enqueue(data) {
            Ok(_) => {
                self.rx_waker.wake();
                Poll::Ready(Ok(()))
            }
            Err((d, e)) => match e {
                EnqueueError::WouldBlock => {
                    self.data.replace(d);
                    self.tx_waker.register(cx.waker());

                    Poll::Pending
                }
                EnqueueError::Closed => Poll::Ready(Err((d, e))),
            },
        }
    }
}

impl<'queue, T> Debug for EnqueueFuture<'queue, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Enqueue-Future ()")
    }
}

impl<'queue, T> Unpin for DequeueFuture<'queue, T> {}

impl<'queue, T> Future for DequeueFuture<'queue, T> {
    type Output = Result<T, DequeueError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Self::Output> {
        match self.queue.try_dequeue() {
            Ok(d) => {
                self.tx_waker.wake();
                Poll::Ready(Ok(d))
            }
            Err(e) => match e {
                DequeueError::WouldBlock => {
                    self.rx_waker.register(cx.waker());
                    Poll::Pending
                }
                DequeueError::Closed => Poll::Ready(Err(DequeueError::Closed)),
            },
        }
    }
}

impl<'queue, T> Debug for DequeueFuture<'queue, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dequeue-Future ()")
    }
}

/// Creates an async BoundedQueue and returns its respecitive
/// ([`AsyncBoundedReceiver`], [`AsyncBoundedSender`])
pub fn async_queue<T>(size: usize) -> (AsyncBoundedReceiver<T>, AsyncBoundedSender<T>) {
    let (u_rx, u_tx) = super::queue(size);

    let rx_waker = Arc::new(AtomicWaker::new());
    let tx_waker = Arc::new(AtomicWaker::new());

    (
        AsyncBoundedReceiver {
            rx_waker: rx_waker.clone(),
            tx_waker: tx_waker.clone(),
            queue: u_rx,
        },
        AsyncBoundedSender {
            rx_waker,
            tx_waker,
            queue: u_tx,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enqueue_dequeue() {
        let (mut rx, mut tx) = async_queue::<usize>(10);

        tx.enqueue(13).await.unwrap();
        assert_eq!(Ok(13), rx.dequeue().await);
    }
}
