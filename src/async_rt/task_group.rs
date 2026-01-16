use super::Executor;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

pub struct TaskGroup {
    inner: Arc<TaskGroupInner>,
}

struct TaskGroupInner {
    exec: Executor,
    active: AtomicUsize,
    join_waker: Mutex<Option<Waker>>,
}

impl TaskGroup {
    pub fn new(exec: Executor) -> Self {
        Self {
            inner: Arc::new(TaskGroupInner {
                exec,
                active: AtomicUsize::new(0),
                join_waker: Mutex::new(None),
            }),
        }
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.inner.active.fetch_add(1, Ordering::AcqRel);
        let st = self.inner.clone();
        let exec = st.exec.clone();
        exec.spawn(async move {
            fut.await;
            if st.active.fetch_sub(1, Ordering::AcqRel) == 1 {
                if let Some(w) = st.join_waker.lock().unwrap().take() {
                    w.wake();
                }
            }
        });
    }

    pub async fn join(&self) {
        JoinFuture {
            st: self.inner.clone(),
        }
        .await
    }
}

struct JoinFuture {
    st: Arc<TaskGroupInner>,
}

impl Future for JoinFuture {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.st.active.load(Ordering::Acquire) == 0 {
            return Poll::Ready(());
        }
        *self.st.join_waker.lock().unwrap() = Some(cx.waker().clone());
        Poll::Pending
    }
}
