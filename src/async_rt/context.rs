use crate::runtime::EventLoop;

use std::cell::Cell;
use std::future::Future;

thread_local! {
    static CURRENT_LOOP: Cell<*mut EventLoop> = const { Cell::new(std::ptr::null_mut()) };
}

pub(crate) struct LoopGuard {
    prev: *mut EventLoop,
}

impl LoopGuard {
    pub(crate) fn enter(loop_ptr: *mut EventLoop) -> Self {
        let prev = CURRENT_LOOP.with(|c| {
            let prev = c.get();
            c.set(loop_ptr);
            prev
        });
        Self { prev }
    }
}

impl Drop for LoopGuard {
    fn drop(&mut self) {
        CURRENT_LOOP.with(|c| c.set(self.prev));
    }
}

pub(crate) fn with_current_loop<R>(f: impl FnOnce(&mut EventLoop) -> R) -> R {
    let ptr = CURRENT_LOOP.with(|c| c.get());
    assert!(
        !ptr.is_null(),
        "async_rt: not running inside EventLoop context"
    );
    unsafe { f(&mut *ptr) }
}

pub fn current_executor() -> super::Executor {
    with_current_loop(|loop_ref| super::Executor::new(loop_ref.handle()))
}

pub fn spawn<F, T>(fut: F) -> super::JoinHandle<T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    current_executor().spawn(fut)
}
