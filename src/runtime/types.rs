#[derive(Debug, Clone, Copy)]
pub enum BackendKind {
    Epoll,
    Poll,
}

#[derive(Debug, Clone, Copy)]
pub enum Interest {
    Readable,
    Writable,
    ReadWrite,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Ready {
    pub readable: bool,
    pub writable: bool,
    pub error: bool,
    pub hup: bool,
}

pub(crate) type Task = Box<dyn FnOnce(&mut crate::runtime::EventLoop) + Send + 'static>;
