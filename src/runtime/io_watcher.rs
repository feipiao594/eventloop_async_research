use super::Handle;
use std::io;
use std::os::unix::io::RawFd;

pub struct IoWatcher {
    fd: RawFd,
    handle: Handle,
    active: bool,
}

impl IoWatcher {
    pub(crate) fn new(fd: RawFd, handle: Handle) -> Self {
        Self {
            fd,
            handle,
            active: true,
        }
    }

    pub fn stop(mut self) -> io::Result<()> {
        self.active = false;
        let fd = self.fd;
        self.handle.post(move |loop_ref| {
            let _ = loop_ref.remove_io(fd);
        })
    }
}

impl Drop for IoWatcher {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let fd = self.fd;
        let _ = self.handle.post(move |loop_ref| {
            let _ = loop_ref.remove_io(fd);
        });
    }
}
