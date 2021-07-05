use futures::task::AtomicWaker;
use std::{fmt::Debug, future::Future, sync::Arc, task::Poll};

use crate::queues::mpsc::{DequeueError, EnqueueError};

use super::{queue, Receiver, Sender};

/// This is the asynchronous Version of the [`Jiffy-Receiver`](Receiver)
pub struct AsyncReceiver<T> {
    /// The shared Waker to inform this receiver of any newly enqueued items
    waker: Arc<AtomicWaker>,
    /// The actual underlying Queue
    queue: Receiver<T>,
}

/// This is the asynchronous Version of the [`Jiffy-Sender`](Sender)
pub struct AsyncSender<T> {
    /// The shared Waker to wake up the Receiver if it is still waiting for
    /// an new Item to be enqueued
    waker: Arc<AtomicWaker>,
    /// The actual underlying Queue
    queue: Sender<T>,
}

impl<T> AsyncReceiver<T> {
    /// Checks if the current Queue has been closed by the Producer
    ///
    /// # Note
    /// This does not mean, that there are no more Elements in the Queue
    /// currently.
    /// It only indicates that there will be no new Elements inserted into the
    /// Queue, but there might still be a any number of Elements currently
    /// still left in the Queue.
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    /// This attempts to dequeue the first Element in the Queue.
    ///
    /// This is the same as [`try_dequeue`](Receiver::try_dequeue) on the
    /// normal Jiffy-Queue
    pub fn try_dequeue(&mut self) -> Result<T, DequeueError> {
        // Simply attempt to dequeue the first Item
        self.queue.try_dequeue()
    }

    /// This is the asynchronous version of the blocking
    /// [`dequeue`](Receiver::dequeue) operation on the normal Jiffy-Queue
    ///
    /// # Behaviour
    /// The Future returned, will either resolve once an item is ready to be
    /// dequeued (`Ok`) or the Queue encountered a "fatal" error, like when the other
    /// side closes the Queue (`Err`)
    ///
    /// # Example
    /// ```
    /// # use nolock::queues::mpsc::jiffy;
    ///
    /// async fn demo() {
    ///   let (mut rx, tx) = jiffy::async_queue::<usize>();
    ///
    ///   tx.enqueue(13).unwrap();
    ///
    ///   assert_eq!(Ok(13), rx.dequeue().await);
    /// }
    ///
    /// # fn main() {
    /// #   let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    /// #
    /// #   rt.block_on(demo());
    /// # }
    /// ```
    pub fn dequeue<'queue>(&'queue mut self) -> DequeueFuture<'queue, T> {
        // Return the right DequeueFuture
        DequeueFuture {
            waker: &self.waker,
            queue: &mut self.queue,
        }
    }
}

impl<T> Debug for AsyncReceiver<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Receiver ()")
    }
}

/// This is the Future returend by the [`Dequeue`](AsyncReceiver::<T>::dequeue)
/// operation on the [`AsyncReceiver`]
///
/// # Behaviour
/// The Future only resolves when it managed to successfully dequeue an Element
/// from the Queue, which will then return `Ok(data)`, or if it encountered a
/// "fatal" [`DequeueError`], like when the Queue has been closed, which will
/// then resolve to `Err(DequeueError)`.
pub struct DequeueFuture<'queue, T> {
    /// This is the Waker on which we will be notified in case the Sender will
    /// enqueue a new Item in the Queue
    waker: &'queue AtomicWaker,
    /// The actual underlying Queue from which we will dequeue the Item
    queue: &'queue mut Receiver<T>,
}

impl<'queue, T> Future for DequeueFuture<'queue, T> {
    type Output = Result<T, DequeueError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // Attempt to Dequeue an Item
        match self.queue.try_dequeue() {
            // If it worked, simply return Ready with the Data as the Result
            Ok(d) => Poll::Ready(Ok(d)),
            // If it did not work, update the Waker and return Pending
            Err(e) => match e {
                DequeueError::WouldBlock => {
                    // Update the shared Waker with the right Waker for the current
                    // Task
                    self.waker.register(cx.waker());

                    // Indicate the we are still waiting for data
                    Poll::Pending
                }
                DequeueError::Closed => Poll::Ready(Err(DequeueError::Closed)),
            },
        }
    }
}

impl<'queue, T> Debug for DequeueFuture<'queue, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Dequeue-Operation ()")
    }
}

impl<T> AsyncSender<T> {
    /// Checks if the Queue has been closed by the Consumer
    pub fn is_closed(&self) -> bool {
        self.queue.is_closed()
    }

    /// Enqueues the given Data
    ///
    /// # Example:
    /// ```
    /// # use nolock::queues::mpsc::jiffy;
    ///
    /// async fn demo() {
    ///   let (mut rx, tx) = jiffy::async_queue::<usize>();
    ///
    ///   // Enqueue the given Data
    ///   assert_eq!(Ok(()), tx.enqueue(13));
    ///   
    ///   # assert_eq!(Ok(13), rx.dequeue().await);
    /// }
    ///
    /// # fn main() {
    /// #   let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    /// #
    /// #   rt.block_on(demo());
    /// # }
    /// ```
    pub fn enqueue(&self, data: T) -> Result<(), (T, EnqueueError)> {
        // Enqueue the Data on the underlying Queue itself
        self.queue.enqueue(data)?;

        // Notify the Receiver about new Data
        self.waker.wake();
        Ok(())
    }
}

impl<T> Debug for AsyncSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Async-Sender ()")
    }
}

/// Creates an async Jiffy-Queue Pair of ([`AsyncReceiver`], [`AsyncSender`])
pub fn async_queue<T>() -> (AsyncReceiver<T>, AsyncSender<T>) {
    let (u_rx, u_tx) = queue();
    let waker = Arc::new(AtomicWaker::new());

    (
        AsyncReceiver {
            waker: waker.clone(),
            queue: u_rx,
        },
        AsyncSender { waker, queue: u_tx },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn enqueue_dequeue() {
        let (mut rx, tx) = async_queue();

        tx.enqueue(13).unwrap();
        assert_eq!(Ok(13), rx.dequeue().await);
    }
}
