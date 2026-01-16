use super::context::with_current_loop;

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

pub fn sleep(delay: Duration) -> Sleep {
    Sleep {
        delay,
        fired: Arc::new(AtomicBool::new(false)),
        waker: Arc::new(Mutex::new(None)),
        started: false,
    }
}

pub struct Sleep {
    delay: Duration,
    fired: Arc<AtomicBool>,
    waker: Arc<Mutex<Option<Waker>>>,
    started: bool,
}

impl Future for Sleep {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.fired.load(Ordering::Acquire) {
            return Poll::Ready(());
        }

        *self.waker.lock().unwrap() = Some(cx.waker().clone());

        if !self.started {
            self.started = true;
            let fired = self.fired.clone();
            let waker = self.waker.clone();
            let delay = self.delay;
            with_current_loop(|loop_ref| {
                loop_ref.post_delayed(delay, move |_| {
                    fired.store(true, Ordering::Release);
                    if let Some(w) = waker.lock().unwrap().take() {
                        w.wake();
                    }
                });
            });
        }

        Poll::Pending
    }
}
