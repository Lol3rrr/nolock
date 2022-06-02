use alloc::sync::Arc;
use futures::task::AtomicWaker;

use crate::queues::DequeueError;

use super::{queue, Receiver, Sender};

/// The sending site of the queue
pub struct AsyncSender<T> {
    sender: Sender<T>,
    waker: Arc<AtomicWaker>,
}

/// The receiving site of the queue
pub struct AsyncReceiver<T> {
    recv: Receiver<T>,
    waker: Arc<AtomicWaker>,
}

/// Creates a new asynchronous Queue
pub fn async_queue<T>() -> (AsyncReceiver<T>, AsyncSender<T>) {
    let (raw_recv, raw_send) = queue::<T>();

    let waker = Arc::new(AtomicWaker::new());

    let recv = AsyncReceiver {
        recv: raw_recv,
        waker: waker.clone(),
    };
    let send = AsyncSender {
        sender: raw_send,
        waker,
    };

    (recv, send)
}

impl<T> AsyncSender<T> {
    /// TODO
    pub fn enqueue(&self, data: T) -> Result<(), T> {
        self.sender.enqueue(data)?;
        self.waker.wake();

        Ok(())
    }
}

impl<T> AsyncReceiver<T> {
    /// TODO
    pub fn try_dequeue(&self) -> Result<T, DequeueError> {
        self.recv.try_dequeue()
    }

    /// TODO
    pub fn dequeue(&self) -> DequeueFuture<'_, T> {
        DequeueFuture {
            recv: &self.recv,
            waker: &self.waker,
        }
    }
}

pub struct DequeueFuture<'s, T> {
    recv: &'s Receiver<T>,
    waker: &'s AtomicWaker,
}

impl<'s, T> core::future::Future for DequeueFuture<'s, T> {
    type Output = Result<T, DequeueError>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        match self.recv.try_dequeue() {
            Ok(r) => return core::task::Poll::Ready(Ok(r)),
            Err(DequeueError::Empty) => {}
            Err(e) => return core::task::Poll::Ready(Err(e)),
        };

        self.waker.register(cx.waker());

        match self.recv.try_dequeue() {
            Ok(r) => return core::task::Poll::Ready(Ok(r)),
            Err(DequeueError::Empty) => {}
            Err(e) => return core::task::Poll::Ready(Err(e)),
        };

        core::task::Poll::Pending
    }
}

#[cfg(test)]
mod tests {
    use core::sync::atomic::AtomicBool;

    use atomic::Ordering;

    use super::*;

    #[test]
    fn create_queue() {
        let (_, _) = async_queue::<i32>();
    }

    #[tokio::test]
    async fn enqueue_dequeue() {
        let (recv, send) = async_queue();

        assert_eq!(Ok(()), send.enqueue(10));

        assert_eq!(Ok(10), recv.dequeue().await);
    }

    #[tokio::test]
    async fn dequeue_enqueue() {
        let (recv, send) = async_queue();

        let woken = Arc::new(AtomicBool::new(false));

        let wok = woken.clone();
        tokio::spawn(async move {
            assert_eq!(Ok(10), recv.dequeue().await);

            wok.store(true, Ordering::SeqCst);
        });

        assert_eq!(Ok(()), send.enqueue(10));

        tokio::task::yield_now().await;

        assert!(woken.load(Ordering::SeqCst));
    }
}
