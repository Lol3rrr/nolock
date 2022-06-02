use alloc::sync::Arc;

use crate::queues::DequeueError;

use super::{queue, Receiver, Sender};

// TODO
// Currently we are only using a single Waker for the Queue, like in the other Queue implementations as well
// but this won't really work in this case, because we can have more than one consumer for the Queue, which
// would overwrite the Wakers of each other.
// We would need some way to wake up all the currently waiting receivers or at least keep track of them and
// notify them one by one
//
// IDEA:
// We could potentially have an append only list of Wakers, where we can't remove the List-Nodes themselves
// but only reuse them for later use to limit memory usage. This would allow us to easily keep track of all
// the currently waiting Wakers and notify them without overwriting each other. And this would also give us
// the opportunity to choose how many receivers we want to wake up each time.

mod waker_list {
    use core::{
        sync::atomic::{AtomicPtr, AtomicU8},
        task::Waker,
    };

    use atomic::Ordering;
    use futures::task::AtomicWaker;

    /// A Lock-Free append-only linked list to store a list of Wakers
    pub struct WakerList {
        head: AtomicPtr<ListEntry>,
    }

    impl WakerList {
        pub fn new() -> Self {
            Self {
                head: AtomicPtr::new(core::ptr::null_mut()),
            }
        }

        /// Registers the Waker on the List
        pub fn register_waker(&self, waker: &Waker) {
            let mut current_ptr = self.head.load(Ordering::SeqCst) as *const ListEntry;
            while !current_ptr.is_null() {
                let current = unsafe { &*current_ptr };

                if current.is_free() && current.try_repopulate(waker) {
                    return;
                }

                current_ptr = current.next;
            }

            let head = self.head.load(Ordering::SeqCst);
            let n_entry = Box::new(ListEntry {
                used: AtomicU8::new(2),
                waker: AtomicWaker::new(),
                next: head,
            });
            n_entry.waker.register(waker);

            let entry_ptr = Box::into_raw(n_entry);
            let mut prev_head = head;

            loop {
                match self.head.compare_exchange(
                    prev_head,
                    entry_ptr,
                    Ordering::SeqCst,
                    Ordering::SeqCst,
                ) {
                    Ok(_) => return,
                    Err(n_head) => {
                        let entry = unsafe { &mut *entry_ptr };
                        entry.next = n_head;

                        prev_head = n_head;
                    }
                };
            }
        }

        /// Wakes up all the currently registered Wakers
        pub fn wakeup_all(&self) {
            let mut current_ptr = self.head.load(Ordering::SeqCst) as *const ListEntry;
            while !current_ptr.is_null() {
                let current = unsafe { &*current_ptr };

                current.try_wakeup();

                current_ptr = current.next;
            }
        }
    }

    struct ListEntry {
        used: AtomicU8,
        waker: AtomicWaker,
        next: *const Self,
    }

    impl ListEntry {
        pub fn try_wakeup(&self) {
            if self.used.load(Ordering::SeqCst) != 2 {
                return;
            }

            if self
                .used
                .compare_exchange(2, 0, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return;
            }

            self.waker.wake();
        }

        pub fn is_free(&self) -> bool {
            self.used.load(Ordering::SeqCst) == 0
        }

        pub fn try_repopulate(&self, waker: &Waker) -> bool {
            if self
                .used
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return false;
            }

            self.waker.register(waker);
            self.used.store(2, Ordering::SeqCst);

            true
        }
    }
}

/// The sending site of the queue
pub struct AsyncSender<T> {
    sender: Sender<T>,
    wakers: Arc<waker_list::WakerList>,
}

/// The receiving site of the queue
pub struct AsyncReceiver<T> {
    recv: Receiver<T>,
    wakers: Arc<waker_list::WakerList>,
}

/// Creates a new asynchronous Queue
pub fn async_queue<T>() -> (AsyncReceiver<T>, AsyncSender<T>) {
    let (raw_recv, raw_send) = queue::<T>();

    let wakers = Arc::new(waker_list::WakerList::new());

    let recv = AsyncReceiver {
        recv: raw_recv,
        wakers: wakers.clone(),
    };
    let send = AsyncSender {
        sender: raw_send,
        wakers,
    };

    (recv, send)
}

impl<T> AsyncSender<T> {
    /// TODO
    pub fn enqueue(&self, data: T) -> Result<(), T> {
        self.sender.enqueue(data)?;
        self.wakers.wakeup_all();

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
            wakers: &self.wakers,
        }
    }
}

pub struct DequeueFuture<'s, T> {
    recv: &'s Receiver<T>,
    wakers: &'s waker_list::WakerList,
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

        self.wakers.register_waker(cx.waker());

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
    use core::sync::atomic::{AtomicBool, AtomicU64};

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

        tokio::task::yield_now().await;

        assert_eq!(Ok(()), send.enqueue(10));

        tokio::task::yield_now().await;

        assert!(woken.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn multiple_enq_deq() {
        let (recv, send) = async_queue();
        let recv = Arc::new(recv);

        let woken = Arc::new(AtomicU64::new(0));

        let wok = woken.clone();
        let rec = recv.clone();
        tokio::spawn(async move {
            assert_eq!(Ok(10), rec.dequeue().await);

            wok.fetch_add(1, Ordering::SeqCst);
        });

        tokio::task::yield_now().await;

        assert_eq!(Ok(()), send.enqueue(10));

        tokio::task::yield_now().await;

        let wok = woken.clone();
        let rec = recv.clone();
        tokio::spawn(async move {
            assert_eq!(Ok(10), rec.dequeue().await);

            wok.fetch_add(1, Ordering::SeqCst);
        });

        tokio::task::yield_now().await;

        assert_eq!(Ok(()), send.enqueue(10));

        tokio::task::yield_now().await;

        assert_eq!(2, woken.load(Ordering::SeqCst));
    }
}
