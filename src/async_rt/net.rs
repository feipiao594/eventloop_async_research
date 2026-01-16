use super::async_fd::AsyncFd;

use std::io;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener as StdTcpListener, TcpStream as StdTcpStream};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct TcpListener {
    inner: Arc<TcpListenerInner>,
}

struct TcpListenerInner {
    listener: Mutex<StdTcpListener>,
    afd: AsyncFd,
}

impl TcpListener {
    pub fn bind(addr: SocketAddr) -> io::Result<Self> {
        let listener = StdTcpListener::bind(addr)?;
        listener.set_nonblocking(true)?;
        let fd = listener.as_raw_fd();
        let afd = AsyncFd::new(fd)?;
        Ok(Self {
            inner: Arc::new(TcpListenerInner {
                listener: Mutex::new(listener),
                afd,
            }),
        })
    }

    pub async fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        loop {
            let res = self.inner.listener.lock().unwrap().accept();
            match res {
                Ok((stream, addr)) => {
                    stream.set_nonblocking(true)?;
                    let s = TcpStream::from_std(stream)?;
                    return Ok((s, addr));
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let _ = self.inner.afd.readable().await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

#[derive(Clone)]
pub struct TcpStream {
    inner: Arc<TcpStreamInner>,
}

struct TcpStreamInner {
    stream: Mutex<StdTcpStream>,
    afd: AsyncFd,
}

impl TcpStream {
    fn from_std(stream: StdTcpStream) -> io::Result<Self> {
        let fd = stream.as_raw_fd();
        let afd = AsyncFd::new(fd)?;
        Ok(Self {
            inner: Arc::new(TcpStreamInner {
                stream: Mutex::new(stream),
                afd,
            }),
        })
    }

    pub async fn recv_some(&self) -> io::Result<Option<Vec<u8>>> {
        let mut buf = vec![0u8; 4096];
        loop {
            let res = self.inner.stream.lock().unwrap().read(&mut buf);
            match res {
                Ok(0) => return Ok(None),
                Ok(n) => {
                    buf.truncate(n);
                    return Ok(Some(buf));
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let _ = self.inner.afd.readable().await;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub async fn send_all(&self, data: &[u8]) -> io::Result<()> {
        let mut offset = 0;
        while offset < data.len() {
            let res = self.inner.stream.lock().unwrap().write(&data[offset..]);
            match res {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "tcp send returned 0",
                    ))
                }
                Ok(n) => offset += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    let _ = self.inner.afd.writable().await;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[allow(dead_code)]
fn _assert_send_sync_fd() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RawFd>();
}
