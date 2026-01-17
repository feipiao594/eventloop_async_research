use super::executor::Task;
use super::Executor;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    Cancelled,
}

pub(crate) struct JoinState<T> {
    pub(crate) result: Mutex<Option<T>>,
    pub(crate) done: AtomicBool,
    pub(crate) cancelled: AtomicBool,
    pub(crate) waker: Mutex<Option<Waker>>,
}

/// A handle to a spawned task that can be awaited for a result.
///
/// Dropping the handle does **not** cancel the task (detached semantics).
pub struct JoinHandle<T> {
    exec: Executor,
    task: Arc<Task>,
    state: Arc<JoinState<T>>,
}

impl<T> JoinHandle<T> {
    pub(crate) fn new(exec: Executor, task: Arc<Task>, state: Arc<JoinState<T>>) -> Self {
        Self { exec, task, state }
    }

    /// Request cancellation of the task.
    pub fn abort(&self) {
        self.state.cancelled.store(true, Ordering::Release);
        self.task.cancel();
        self.exec.schedule(self.task.clone());
        if let Some(w) = self.state.waker.lock().unwrap().take() {
            w.wake();
        }
    }

    pub fn is_finished(&self) -> bool {
        self.state.done.load(Ordering::Acquire)
    }
}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = Result<T, JoinError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.state.cancelled.load(Ordering::Acquire) {
            return Poll::Ready(Err(JoinError::Cancelled));
        }

        if let Some(v) = self.state.result.lock().unwrap().take() {
            self.state.done.store(true, Ordering::Release);
            return Poll::Ready(Ok(v));
        }

        *self.state.waker.lock().unwrap() = Some(cx.waker().clone());
        Poll::Pending
    }
}

pub(crate) fn join_state<T>() -> Arc<JoinState<T>> {
    Arc::new(JoinState {
        result: Mutex::new(None),
        done: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
        waker: Mutex::new(None),
    })
}

pub async fn join_all<T>(handles: Vec<JoinHandle<T>>) -> Vec<Result<T, JoinError>>
where
    T: Send + 'static,
{
    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        out.push(h.await);
    }
    out
}

pub enum Select2<T> {
    A(Result<T, JoinError>),
    B(Result<T, JoinError>),
}

pub async fn select2<T>(a: JoinHandle<T>, b: JoinHandle<T>) -> Select2<T>
where
    T: Send + 'static,
{
    Select2Future { a, b }.await
}

pub struct SelectAny<T> {
    pub index: usize,
    pub result: Result<T, JoinError>,
    pub remaining: Vec<JoinHandle<T>>,
}

pub async fn select_any<T>(handles: Vec<JoinHandle<T>>) -> SelectAny<T>
where
    T: Send + 'static,
{
    assert!(!handles.is_empty(), "select_any requires at least 1 handle");
    SelectAnyFuture { handles }.await
}

struct SelectAnyFuture<T> {
    handles: Vec<JoinHandle<T>>,
}

impl<T: Send + 'static> Future for SelectAnyFuture<T> {
    type Output = SelectAny<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        for i in 0..self.handles.len() {
            let handle = &mut self.handles[i];
            if let Poll::Ready(result) = Pin::new(handle).poll(cx) {
                let mut remaining = std::mem::take(&mut self.handles);
                remaining.remove(i);
                return Poll::Ready(SelectAny {
                    index: i,
                    result,
                    remaining,
                });
            }
        }
        Poll::Pending
    }
}

struct Select2Future<T> {
    a: JoinHandle<T>,
    b: JoinHandle<T>,
}

impl<T: Send + 'static> Future for Select2Future<T> {
    type Output = Select2<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Poll::Ready(r) = Pin::new(&mut self.a).poll(cx) {
            return Poll::Ready(Select2::A(r));
        }
        if let Poll::Ready(r) = Pin::new(&mut self.b).poll(cx) {
            return Poll::Ready(Select2::B(r));
        }
        Poll::Pending
    }
}
