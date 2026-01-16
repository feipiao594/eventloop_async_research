use super::{os, BackendKind, Interest, Ready};
use std::io;
use std::os::unix::io::RawFd;
use std::time::Duration;

pub(crate) enum Backend {
    #[cfg(target_os = "linux")]
    Epoll(os::linux::EpollBackend),
    Poll(os::unix::PollBackend),
}

impl Backend {
    pub(crate) fn new(kind: BackendKind) -> io::Result<Self> {
        match kind {
            BackendKind::Poll => Ok(Self::Poll(os::unix::PollBackend::new()?)),
            BackendKind::Epoll => {
                #[cfg(target_os = "linux")]
                {
                    Ok(Self::Epoll(os::linux::EpollBackend::new()?))
                }
                #[cfg(not(target_os = "linux"))]
                {
                    Err(io::Error::new(
                        io::ErrorKind::Unsupported,
                        "epoll backend is only supported on linux",
                    ))
                }
            }
        }
    }

    pub(crate) fn register(&mut self, fd: RawFd, interest: Interest) -> io::Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Backend::Epoll(b) => b.register(fd, interest),
            Backend::Poll(b) => b.register(fd, interest),
        }
    }

    pub(crate) fn deregister(&mut self, fd: RawFd) -> io::Result<()> {
        match self {
            #[cfg(target_os = "linux")]
            Backend::Epoll(b) => b.deregister(fd),
            Backend::Poll(b) => b.deregister(fd),
        }
    }

    pub(crate) fn wait(&mut self, timeout: Option<Duration>) -> io::Result<Vec<(RawFd, Ready)>> {
        match self {
            #[cfg(target_os = "linux")]
            Backend::Epoll(b) => b.wait(timeout),
            Backend::Poll(b) => b.wait(timeout),
        }
    }
}
