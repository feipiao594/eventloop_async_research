use super::context::with_current_loop;
use crate::runtime::{Interest, IoWatcher, Ready};

use std::future::Future;
use std::io;
use std::os::unix::io::RawFd;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

#[derive(Clone)]
pub struct AsyncFd {
    inner: Arc<AsyncFdInner>,
}

struct AsyncFdInner {
    _watcher: IoWatcher,
    state: Arc<Mutex<AsyncFdState>>,
}

#[derive(Default)]
struct AsyncFdState {
    readable: bool,
    writable: bool,
    error: bool,
    hup: bool,
    read_waker: Option<Waker>,
    write_waker: Option<Waker>,
}

impl AsyncFd {
    pub fn new(fd: RawFd) -> io::Result<Self> {
        let state = Arc::new(Mutex::new(AsyncFdState::default()));
        let state_cb = state.clone();

        let watcher = with_current_loop(|loop_ref| {
            loop_ref.watch_io(fd, Interest::ReadWrite, move |_, ready: Ready| {
                let mut st = state_cb.lock().unwrap();
                st.readable |= ready.readable;
                st.writable |= ready.writable;
                st.error |= ready.error;
                st.hup |= ready.hup;

                if (st.readable || st.error || st.hup) && st.read_waker.is_some() {
                    if let Some(w) = st.read_waker.take() {
                        w.wake();
                    }
                }
                if (st.writable || st.error || st.hup) && st.write_waker.is_some() {
                    if let Some(w) = st.write_waker.take() {
                        w.wake();
                    }
                }
            })
        })?;

        Ok(Self {
            inner: Arc::new(AsyncFdInner {
                _watcher: watcher,
                state,
            }),
        })
    }

    pub async fn readable(&self) -> Ready {
        ReadableFuture { afd: self.clone() }.await
    }

    pub async fn writable(&self) -> Ready {
        WritableFuture { afd: self.clone() }.await
    }
}

struct ReadableFuture {
    afd: AsyncFd,
}

impl Future for ReadableFuture {
    type Output = Ready;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = self.afd.inner.state.lock().unwrap();
        if st.readable || st.error || st.hup {
            let out = Ready {
                readable: st.readable,
                writable: st.writable,
                error: st.error,
                hup: st.hup,
            };
            st.readable = false;
            st.error = false;
            st.hup = false;
            return Poll::Ready(out);
        }
        st.read_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}

struct WritableFuture {
    afd: AsyncFd,
}

impl Future for WritableFuture {
    type Output = Ready;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut st = self.afd.inner.state.lock().unwrap();
        if st.writable || st.error || st.hup {
            let out = Ready {
                readable: st.readable,
                writable: st.writable,
                error: st.error,
                hup: st.hup,
            };
            st.writable = false;
            st.error = false;
            st.hup = false;
            return Poll::Ready(out);
        }
        st.write_waker = Some(cx.waker().clone());
        Poll::Pending
    }
}
