use std::collections::HashMap;
use std::io;
use std::os::raw::{c_int, c_short};
use std::os::unix::io::RawFd;
use std::time::Duration;

use crate::runtime::{Interest, Ready};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct PollFd {
    fd: c_int,
    events: c_short,
    revents: c_short,
}

extern "C" {
    fn poll(fds: *mut PollFd, nfds: usize, timeout_ms: c_int) -> c_int;
}

const POLLIN: c_short = 0x0001;
const POLLOUT: c_short = 0x0004;
const POLLERR: c_short = 0x0008;
const POLLHUP: c_short = 0x0010;

pub struct PollBackend {
    fds: Vec<PollFd>,
    index: HashMap<RawFd, usize>,
}

impl PollBackend {
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            fds: Vec::new(),
            index: HashMap::new(),
        })
    }

    pub fn register(&mut self, fd: RawFd, interest: Interest) -> io::Result<()> {
        if self.index.contains_key(&fd) {
            return Ok(());
        }

        let events = match interest {
            Interest::Readable => POLLIN,
            Interest::Writable => POLLOUT,
            Interest::ReadWrite => POLLIN | POLLOUT,
        };

        let pfd = PollFd {
            fd: fd as c_int,
            events,
            revents: 0,
        };
        let idx = self.fds.len();
        self.fds.push(pfd);
        self.index.insert(fd, idx);
        Ok(())
    }

    pub fn deregister(&mut self, fd: RawFd) -> io::Result<()> {
        let Some(idx) = self.index.remove(&fd) else {
            return Ok(());
        };

        let last = self.fds.len() - 1;
        let moved_fd = self.fds[last].fd as RawFd;
        self.fds.swap(idx, last);
        self.fds.pop();

        if idx != last {
            self.index.insert(moved_fd, idx);
        }
        Ok(())
    }

    pub fn wait(&mut self, timeout: Option<Duration>) -> io::Result<Vec<(RawFd, Ready)>> {
        let timeout_ms = timeout
            .map(|d| d.as_millis().min(i32::MAX as u128) as i32)
            .unwrap_or(-1);

        for p in &mut self.fds {
            p.revents = 0;
        }

        let rc = unsafe { poll(self.fds.as_mut_ptr(), self.fds.len(), timeout_ms as c_int) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }

        let mut out = Vec::new();
        if rc == 0 {
            return Ok(out);
        }

        for p in &self.fds {
            if p.revents == 0 {
                continue;
            }
            out.push((
                p.fd as RawFd,
                Ready {
                    readable: (p.revents & POLLIN) != 0,
                    writable: (p.revents & POLLOUT) != 0,
                    error: (p.revents & POLLERR) != 0,
                    hup: (p.revents & POLLHUP) != 0,
                },
            ));
        }
        Ok(out)
    }
}
