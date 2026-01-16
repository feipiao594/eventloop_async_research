use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Clone)]
pub struct AsyncQueue<T> {
    inner: Arc<Mutex<AsyncQueueInner<T>>>,
}

struct AsyncQueueInner<T> {
    buf: VecDeque<T>,
    closed: bool,
    waiter: Option<Waker>,
}

impl<T> AsyncQueue<T> {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(AsyncQueueInner {
                buf: VecDeque::new(),
                closed: false,
                waiter: None,
            })),
        }
    }

    pub fn push(&self, item: T) {
        let mut st = self.inner.lock().unwrap();
        if st.closed {
            return;
        }
        st.buf.push_back(item);
        if let Some(w) = st.waiter.take() {
            w.wake();
        }
    }

    pub fn close(&self) {
        let mut st = self.inner.lock().unwrap();
        st.closed = true;
        if let Some(w) = st.waiter.take() {
            w.wake();
        }
    }

    pub async fn pop(&self) -> Option<T> {
        PopFuture {
            q: AsyncQueue {
                inner: self.inner.clone(),
            },
        }
        .await
    }
}

struct PopFuture<T> {
    q: AsyncQueue<T>,
}

impl<T> Future for PopFuture<T> {
    type Output = Option<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = self.q.inner.lock().unwrap();
        if let Some(v) = st.buf.pop_front() {
            return Poll::Ready(Some(v));
        }
        if st.closed {
            return Poll::Ready(None);
        }
        st.waiter = Some(cx.waker().clone());
        Poll::Pending
    }
}
