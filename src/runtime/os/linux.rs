use std::io;
use std::os::raw::c_int;
use std::os::unix::io::RawFd;
use std::time::Duration;

use crate::runtime::{Interest, Ready};

#[repr(C)]
#[derive(Clone, Copy)]
#[repr(C, packed)]
struct EpollEvent {
    events: u32,
    data: u64,
}

extern "C" {
    fn epoll_create1(flags: c_int) -> c_int;
    fn epoll_ctl(epfd: c_int, op: c_int, fd: c_int, event: *mut EpollEvent) -> c_int;
    fn epoll_wait(epfd: c_int, events: *mut EpollEvent, maxevents: c_int, timeout: c_int) -> c_int;
    fn close(fd: c_int) -> c_int;
}

const EPOLL_CLOEXEC: c_int = 0x80000;

const EPOLL_CTL_ADD: c_int = 1;
const EPOLL_CTL_DEL: c_int = 2;

const EPOLLIN: u32 = 0x001;
const EPOLLOUT: u32 = 0x004;
const EPOLLERR: u32 = 0x008;
const EPOLLHUP: u32 = 0x010;

pub struct EpollBackend {
    epfd: RawFd,
    events: Vec<EpollEvent>,
}

impl EpollBackend {
    pub fn new() -> io::Result<Self> {
        let epfd = unsafe { epoll_create1(EPOLL_CLOEXEC) };
        if epfd < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            epfd,
            events: vec![EpollEvent { events: 0, data: 0 }; 1024],
        })
    }

    pub fn register(&mut self, fd: RawFd, interest: Interest) -> io::Result<()> {
        let events = match interest {
            Interest::Readable => EPOLLIN,
            Interest::Writable => EPOLLOUT,
            Interest::ReadWrite => EPOLLIN | EPOLLOUT,
        };
        let mut ev = EpollEvent {
            events,
            data: fd as u64,
        };
        let rc = unsafe { epoll_ctl(self.epfd, EPOLL_CTL_ADD, fd as c_int, &mut ev) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn deregister(&mut self, fd: RawFd) -> io::Result<()> {
        let rc = unsafe { epoll_ctl(self.epfd, EPOLL_CTL_DEL, fd as c_int, std::ptr::null_mut()) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<Vec<(RawFd, Ready)>> {
        let timeout_ms = timeout
            .map(|d| d.as_millis().min(i32::MAX as u128) as i32)
            .unwrap_or(-1);

        let rc = unsafe {
            epoll_wait(
                self.epfd,
                self.events.as_mut_ptr(),
                self.events.len() as c_int,
                timeout_ms as c_int,
            )
        };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut out = Vec::new();
        for ev in self.events.iter().take(rc as usize) {
            let bits = ev.events;
            let data = unsafe { std::ptr::read_unaligned(std::ptr::addr_of!(ev.data)) };
            out.push((
                data as RawFd,
                Ready {
                    readable: (bits & EPOLLIN) != 0,
                    writable: (bits & EPOLLOUT) != 0,
                    error: (bits & EPOLLERR) != 0,
                    hup: (bits & EPOLLHUP) != 0,
                },
            ));
        }
        Ok(out)
    }
}

impl Drop for EpollBackend {
    fn drop(&mut self) {
        unsafe {
            let _ = close(self.epfd as c_int);
        }
    }
}
