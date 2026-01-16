use std::fs::File;
use std::io::{self, Write};
use std::os::raw::c_int;
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct Waker {
    writer: Arc<File>,
}

impl Waker {
    pub(crate) fn wake(&self) -> io::Result<()> {
        let _ = (&*self.writer).write(&[1u8]);
        Ok(())
    }
}

pub(crate) fn make_waker() -> io::Result<(File, Waker)> {
    let (reader, writer) = make_pipe()?;
    set_nonblocking(reader.as_raw_fd())?;
    let waker = Waker {
        writer: Arc::new(writer),
    };
    Ok((reader, waker))
}

fn make_pipe() -> io::Result<(File, File)> {
    let mut fds = [0 as c_int, 0 as c_int];

    #[cfg(target_os = "linux")]
    unsafe {
        if pipe2(fds.as_mut_ptr(), O_CLOEXEC | O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    #[cfg(not(target_os = "linux"))]
    unsafe {
        if pipe(fds.as_mut_ptr()) < 0 {
            return Err(io::Error::last_os_error());
        }
    }

    let reader = unsafe { File::from_raw_fd(fds[0]) };
    let writer = unsafe { File::from_raw_fd(fds[1]) };
    Ok((reader, writer))
}

fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    unsafe {
        let flags = fcntl(fd as c_int, F_GETFL);
        if flags < 0 {
            return Err(io::Error::last_os_error());
        }
        if fcntl(fd as c_int, F_SETFL, flags | O_NONBLOCK) < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

extern "C" {
    #[cfg(target_os = "linux")]
    fn pipe2(fds: *mut c_int, flags: c_int) -> c_int;
    #[cfg(not(target_os = "linux"))]
    fn pipe(fds: *mut c_int) -> c_int;

    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
}

const F_GETFL: c_int = 3;
const F_SETFL: c_int = 4;

#[cfg(target_os = "linux")]
const O_NONBLOCK: c_int = 0o0004000;
#[cfg(not(target_os = "linux"))]
const O_NONBLOCK: c_int = 0x0004;

#[cfg(target_os = "linux")]
const O_CLOEXEC: c_int = 0o2000000;
