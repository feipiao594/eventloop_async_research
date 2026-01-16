pub mod async_rt;
pub mod runtime;

pub use eventloop_async_research_macros::main;
pub use runtime::{BackendKind, EventLoop, Handle, Interest, IoWatcher, Ready};

pub fn default_backend() -> BackendKind {
    if cfg!(target_os = "linux") {
        BackendKind::Epoll
    } else {
        BackendKind::Poll
    }
}

pub fn backend_from_args() -> BackendKind {
    match std::env::args().nth(1).as_deref() {
        Some("poll") => BackendKind::Poll,
        Some("epoll") => BackendKind::Epoll,
        _ => default_backend(),
    }
}

pub fn run<F, R>(backend: BackendKind, fut: F) -> std::io::Result<R>
where
    F: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
{
    use std::sync::{Arc, Mutex};

    let (mut event_loop, handle) = EventLoop::new(backend)?;
    let exec = async_rt::Executor::new(handle.clone());

    let out = Arc::new(Mutex::new(None::<R>));
    let out2 = out.clone();

    exec.spawn(async move {
        let result = fut.await;
        *out2.lock().unwrap() = Some(result);
        let _ = handle.post(|loop_ref| loop_ref.request_exit());
    });

    event_loop.run();
    let result = out
        .lock()
        .unwrap()
        .take()
        .expect("rt exited without result");
    Ok(result)
}

#[macro_export]
macro_rules! rt_main {
    (async fn main() $body:block) => {
        fn main() {
            $crate::run($crate::backend_from_args(), async move $body).unwrap();
        }
    };
    (backend = $backend:expr, async fn main() $body:block) => {
        fn main() {
            $crate::run($backend, async move $body).unwrap();
        }
    };
    (async fn main() -> () $body:block) => {
        fn main() {
            $crate::run($crate::backend_from_args(), async move $body).unwrap();
        }
    };
    (backend = $backend:expr, async fn main() -> () $body:block) => {
        fn main() {
            $crate::run($backend, async move $body).unwrap();
        }
    };
    (async fn main() -> std::io::Result<$ok:ty> $body:block) => {
        fn main() -> std::io::Result<$ok> {
            $crate::run($crate::backend_from_args(), async move $body)?
        }
    };
    (backend = $backend:expr, async fn main() -> std::io::Result<$ok:ty> $body:block) => {
        fn main() -> std::io::Result<$ok> {
            $crate::run($backend, async move $body)?
        }
    };
    (async fn main() -> $ret:ty $body:block) => {
        fn main() -> $ret {
            $crate::run($crate::backend_from_args(), async move $body)
        }
    };
    (backend = $backend:expr, async fn main() -> $ret:ty $body:block) => {
        fn main() -> $ret {
            $crate::run($backend, async move $body)
        }
    };
}
