use std::{fmt::Debug, future::Future, sync::Arc, task::Poll};

use futures::task::AtomicWaker;

use crate::queues::{DequeueError, EnqueueError};

use super::{queue, UnboundedReceiver, UnboundedSender};

/// This is the async Variant of the [`UnboundedSender`].
///
/// Created using [`async_queue`]
pub struct AsyncUnboundedSender<T> {
    rx_waker: Arc<AtomicWaker>,
    queue: UnboundedSender<T>,
}

/// This is the async Variant of the [`UnboundedReceiver`].
///
/// Created using [`async_queue`]
pub struct AsyncUnboundedReceiver<T> {
    rx_waker: Arc<AtomicWaker>,
    queue: UnboundedReceiver<T>,
}

/// The Future returned by the
/// [`dequeue`](AsyncUnboundedReceiver::dequeue)-Operation
///
/// # Behaviour
/// This Future only resolves when it either successfully dequeued an Element
/// and then returns it as `Ok(Element)` or when the Queue has been closed by
/// the Producer and the Queue is currently empty, therefore no more Elements
/// will be added to the Queue meaning that we would wait forever, instead it
/// returns `Err(DequeueError)`
pub struct DequeueFuture<'queue, T> {
    rx_waker: &'queue AtomicWaker,
    queue: &'queue mut UnboundedReceiver<T>,
}

impl<T> AsyncUnboundedSender<T> {
    /// Checks if the Queue has been closed by the Consumer
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    /// Enqueues the given Data on the Queue
    pub fn enqueue(&mut self, data: T) -> Result<(), (T, EnqueueError)> {
        self.queue.enqueue(data)?;
        self.rx_waker.wake();
        Ok(())
    }
}

impl<T> Debug for AsyncUnboundedSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Unbounded-Sender ()")
    }
}

impl<T> AsyncUnboundedReceiver<T> {
    /// Checks if the Queue has been closed by the Producer
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    /// Dequeues the next Item from the Queue
    pub fn dequeue(&mut self) -> DequeueFuture<'_, T> {
        DequeueFuture {
            rx_waker: &self.rx_waker,
            queue: &mut self.queue,
        }
    }

    /// This attempts to dequeue the next Item from the Queue.
    ///
    /// This behaves just like the normal
    /// [`try_dequeue`](UnboundedReceiver::try_dequeue)-Operation
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        self.queue.try_dequeue()
    }
}

impl<T> Debug for AsyncUnboundedReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Unbounded-Receiver ()")
    }
}

impl<'queue, T> Future for DequeueFuture<'queue, T> {
    type Output = Result<T, DequeueError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.queue.try_dequeue() {
            Ok(d) => Poll::Ready(Ok(d)),
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

/// Creates a new async SPSC-Queue and returns its respective
/// ([`AsyncUnboundedReceiver`], [`AsyncUnboundedSender`])
pub fn async_queue<T>() -> (AsyncUnboundedReceiver<T>, AsyncUnboundedSender<T>) {
    let (u_rx, u_tx) = queue();

    let rx_waker = Arc::new(AtomicWaker::new());

    (
        AsyncUnboundedReceiver {
            rx_waker: rx_waker.clone(),
            queue: u_rx,
        },
        AsyncUnboundedSender {
            rx_waker,
            queue: u_tx,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_dequeue() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();

        async fn test_fn() {
            let (mut rx, mut tx) = async_queue();

            tx.enqueue(13).unwrap();
            assert_eq!(Ok(13), rx.dequeue().await);
        }

        runtime.block_on(test_fn());
    }
}
