use super::context::LoopGuard;
use crate::runtime::Handle;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

#[derive(Clone)]
pub struct Executor {
    inner: Arc<ExecutorInner>,
}

struct ExecutorInner {
    handle: Handle,
}

impl Executor {
    pub fn new(handle: Handle) -> Self {
        Self {
            inner: Arc::new(ExecutorInner { handle }),
        }
    }

    pub fn spawn<F>(&self, fut: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let task = Arc::new(Task {
            fut: Mutex::new(Box::pin(fut)),
            scheduled: AtomicBool::new(false),
            done: AtomicBool::new(false),
        });
        self.schedule(task);
    }

    pub(crate) fn schedule(&self, task: Arc<Task>) {
        if task.done.load(Ordering::Acquire) {
            return;
        }
        if task.scheduled.swap(true, Ordering::AcqRel) {
            return;
        }

        let exec = self.clone();
        let _ = self.inner.handle.post(move |loop_ref| {
            let _guard = LoopGuard::enter(loop_ref as *mut _);
            task.scheduled.store(false, Ordering::Release);
            task.poll(&exec);
        });
    }
}

pub(crate) struct Task {
    fut: Mutex<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>,
    scheduled: AtomicBool,
    done: AtomicBool,
}

impl Task {
    fn poll(self: &Arc<Self>, exec: &Executor) {
        if self.done.load(Ordering::Acquire) {
            return;
        }

        let waker = task_waker(exec.clone(), self.clone());
        let mut cx = Context::from_waker(&waker);

        let mut fut = self.fut.lock().unwrap();
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(()) => {
                self.done.store(true, Ordering::Release);
            }
            Poll::Pending => {}
        }
    }
}

struct WakerData {
    exec: Executor,
    task: Arc<Task>,
}

fn task_waker(exec: Executor, task: Arc<Task>) -> Waker {
    let data = Arc::new(WakerData { exec, task });
    unsafe { Waker::from_raw(raw_waker(data)) }
}

fn raw_waker(data: Arc<WakerData>) -> RawWaker {
    let ptr = Arc::into_raw(data) as *const ();
    RawWaker::new(ptr, &VTABLE)
}

static VTABLE: RawWakerVTable =
    RawWakerVTable::new(clone_waker, wake_waker, wake_by_ref_waker, drop_waker);

unsafe fn clone_waker(ptr: *const ()) -> RawWaker {
    let arc = Arc::<WakerData>::from_raw(ptr as *const WakerData);
    let cloned = arc.clone();
    let _ = Arc::into_raw(arc);
    raw_waker(cloned)
}

unsafe fn wake_waker(ptr: *const ()) {
    let arc = Arc::<WakerData>::from_raw(ptr as *const WakerData);
    arc.exec.schedule(arc.task.clone());
}

unsafe fn wake_by_ref_waker(ptr: *const ()) {
    let arc = Arc::<WakerData>::from_raw(ptr as *const WakerData);
    arc.exec.schedule(arc.task.clone());
    let _ = Arc::into_raw(arc);
}

unsafe fn drop_waker(ptr: *const ()) {
    let _ = Arc::<WakerData>::from_raw(ptr as *const WakerData);
}
