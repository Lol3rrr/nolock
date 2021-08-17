use std::{fmt::Debug, future::Future, sync::Arc, task::Poll};

use futures::task::AtomicWaker;

use crate::queues::{DequeueError, EnqueueError};

use super::{BoundedReceiver, BoundedSender};

/// An async variant of the [`BoundedSender`] that allows your to efficiently
/// use this Queue in async Contexts as well.
///
/// Created using the [`async_queue`] method
pub struct AsyncBoundedSender<T> {
    rx_waker: Arc<AtomicWaker>,
    tx_waker: Arc<AtomicWaker>,
    queue: BoundedSender<T>,
}

/// An async variant of the [`BoundedReceiver`] that allows your to efficiently
/// use this Queue in async Contexts as well.
///
/// Created using the [`async_queue`] method
pub struct AsyncBoundedReceiver<T> {
    rx_waker: Arc<AtomicWaker>,
    tx_waker: Arc<AtomicWaker>,
    queue: BoundedReceiver<T>,
}

/// The Future returned when enqueueing an Item
///
/// # Behaviour
/// This Future only resolves when it either successfully enqueued the Item
/// in the Queue (`Ok`) or when the Queue gets closed by the Consumer and
/// therefore no more Items can be enqueued into it (`Err`)
pub struct EnqueueFuture<'queue, T> {
    /// The Waker to notify a potential waiting Dequeue-Operation
    rx_waker: &'queue AtomicWaker,
    /// The Waker for this type of Future
    tx_waker: &'queue AtomicWaker,
    /// The actual underlying Queue
    queue: &'queue mut BoundedSender<T>,
    /// The Data that the User wants to enqueue
    data: Option<T>,
}

/// The Future returned when dequeue an Item
///
/// # Behaviour
/// This Future only resolves when it either successfully dequeued an Item
/// and then returns it (`Ok(item)`) or when the Queue was closed by the Producer
/// and there are no more Items left in it to be dequeued (`Err`)
pub struct DequeueFuture<'queue, T> {
    /// The Waker for this type of Future
    rx_waker: &'queue AtomicWaker,
    /// The Waker to notify a potential waiting Enqueue-Operation
    tx_waker: &'queue AtomicWaker,
    /// The actual underlying Queue
    queue: &'queue mut BoundedReceiver<T>,
}

impl<T> AsyncBoundedSender<T> {
    /// Checks if the Queue has been closed by the Consumer
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    /// The async variant of the blocking [`enqueue`](BoundedSender::enqueue)
    /// operation on the Non-Async version of the Queue
    pub fn enqueue(&mut self, data: T) -> EnqueueFuture<'_, T> {
        EnqueueFuture {
            rx_waker: &self.rx_waker,
            tx_waker: &self.tx_waker,
            queue: &mut self.queue,
            data: Some(data),
        }
    }

    /// Attempts to enqueue the given Data on the Queue.
    ///
    /// This behaves just like the [`try_enqueue`](BoundedSender::try_enqueue)
    /// operation on the normal sync-BoundedSender
    pub fn try_enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
        match self.queue.try_enqueue(data) {
            Ok(_) => {
                self.rx_waker.wake();
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Checks if the Queue is currently Full
    pub fn is_full(&self) -> bool {
        self.queue.is_full()
    }
}

impl<T> Debug for AsyncBoundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Bounded-Sender ()")
    }
}

impl<T> AsyncBoundedReceiver<T> {
    /// Checks if the Queue has been closed by the Producer
    ///
    /// # Note
    /// Even when this indicates that the Queue is closed, there might still be
    /// Items left in the Queue that the Consumer should dequeue first to make
    /// sure that no data is lost
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    /// The async variant of the blocking [`dequeue`](BoundedReceiver::dequeue)
    /// operation on the Non-Async version of the Queue
    pub fn dequeue(&mut self) -> DequeueFuture<'_, T> {
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

    /// Checks if the Queue is currently Empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
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
                EnqueueError::Full => {
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
                DequeueError::Empty => {
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
    #[cfg_attr(miri, ignore)]
    async fn enqueue_dequeue() {
        let (mut rx, mut tx) = async_queue::<usize>(10);

        tx.enqueue(13).await.unwrap();
        assert_eq!(Ok(13), rx.dequeue().await);
    }
}
