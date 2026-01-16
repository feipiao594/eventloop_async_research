use super::waker::Waker;
use super::Task;
use std::io;
use std::sync::mpsc;

#[derive(Clone)]
pub struct Handle {
    pub(crate) tx: mpsc::Sender<Task>,
    pub(crate) waker: Waker,
}

impl Handle {
    pub fn post<F>(&self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut super::EventLoop) + Send + 'static,
    {
        self.tx.send(Box::new(f)).map_err(|_| {
            io::Error::new(io::ErrorKind::BrokenPipe, "event loop task channel closed")
        })?;
        self.waker.wake()
    }
}
