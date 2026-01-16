use super::backend::Backend;
use super::timer::Timer;
use super::waker::make_waker;
use super::{Handle, Interest, IoWatcher, Ready, Task};

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::io::{self, Read};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct Source {
    interest: Interest,
    callback: Box<dyn FnMut(&mut super::EventLoop, Ready) + 'static>,
}

pub struct EventLoop {
    backend: Backend,

    handle: Handle,
    exit_requested: bool,
    in_dispatch: bool,
    trace: bool,

    sources: HashMap<RawFd, Source>,
    pending_add: Vec<(RawFd, Source)>,
    pending_remove: Vec<RawFd>,

    local_tasks: VecDeque<Task>,
    shared_rx: mpsc::Receiver<Task>,

    timers: BinaryHeap<Reverse<Timer>>,
    timer_seq: u64,
}

impl EventLoop {
    pub fn new(kind: super::BackendKind) -> io::Result<(Self, Handle)> {
        let backend = Backend::new(kind)?;
        let (tx, rx) = mpsc::channel::<Task>();

        let (mut reader, waker) = make_waker()?;
        let handle = Handle { tx, waker };

        let mut loop_ref = Self {
            backend,
            handle: handle.clone(),
            exit_requested: false,
            in_dispatch: false,
            trace: std::env::var_os("EVLOOP_TRACE").is_some(),
            sources: HashMap::new(),
            pending_add: Vec::new(),
            pending_remove: Vec::new(),
            local_tasks: VecDeque::new(),
            shared_rx: rx,
            timers: BinaryHeap::new(),
            timer_seq: 0,
        };

        loop_ref.add_io(reader.as_raw_fd(), Interest::Readable, move |_, ready| {
            if !ready.readable {
                return;
            }
            let mut buf = [0u8; 128];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => return,
                    Ok(_) => continue,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                    Err(_) => return,
                }
            }
        })?;

        Ok((loop_ref, handle))
    }

    pub fn handle(&self) -> Handle {
        self.handle.clone()
    }

    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    pub fn post<F>(&mut self, f: F)
    where
        F: FnOnce(&mut super::EventLoop) + Send + 'static,
    {
        self.local_tasks.push_back(Box::new(f));
    }

    pub fn post_delayed<F>(&mut self, delay: Duration, f: F)
    where
        F: FnOnce(&mut super::EventLoop) + Send + 'static,
    {
        let when = Instant::now() + delay;
        let seq = self.timer_seq;
        self.timer_seq = self.timer_seq.wrapping_add(1);
        self.timers.push(Reverse(Timer {
            when,
            seq,
            task: Box::new(f),
        }));
    }

    pub fn add_io<F>(&mut self, fd: RawFd, interest: Interest, callback: F) -> io::Result<()>
    where
        F: FnMut(&mut super::EventLoop, Ready) + 'static,
    {
        let src = Source {
            interest,
            callback: Box::new(callback),
        };

        if self.in_dispatch {
            self.pending_add.push((fd, src));
            return Ok(());
        }

        self.backend.register(fd, interest)?;
        self.sources.insert(fd, src);
        Ok(())
    }

    pub fn watch_io<F>(
        &mut self,
        fd: RawFd,
        interest: Interest,
        callback: F,
    ) -> io::Result<IoWatcher>
    where
        F: FnMut(&mut super::EventLoop, Ready) + 'static,
    {
        self.add_io(fd, interest, callback)?;
        Ok(IoWatcher::new(fd, self.handle()))
    }

    pub fn remove_io(&mut self, fd: RawFd) -> io::Result<()> {
        if self.in_dispatch {
            self.pending_remove.push(fd);
            return Ok(());
        }
        self.backend.deregister(fd)?;
        self.sources.remove(&fd);
        Ok(())
    }

    pub fn run(&mut self) {
        while !self.exit_requested {
            self.drain_shared_tasks();
            self.run_expired_timers();
            self.run_local_tasks();
            if self.exit_requested {
                break;
            }

            let timeout = self.compute_timeout();
            let events = match self.backend.wait(timeout) {
                Ok(ev) => ev,
                Err(e) => {
                    eprintln!("backend wait error: {e}");
                    break;
                }
            };
            if self.trace {
                eprintln!("wait -> {} events: {:?}", events.len(), events);
            }

            self.in_dispatch = true;
            for (fd, ready) in events {
                let Some(mut src) = self.sources.remove(&fd) else {
                    continue;
                };
                (src.callback)(self, ready);
                let remove_requested = self.pending_remove.iter().any(|&x| x == fd);
                if !remove_requested && !self.sources.contains_key(&fd) {
                    self.sources.insert(fd, src);
                }
            }
            self.in_dispatch = false;

            if !self.pending_remove.is_empty() {
                let to_remove = std::mem::take(&mut self.pending_remove);
                for fd in to_remove {
                    let _ = self.remove_io(fd);
                }
            }

            if !self.pending_add.is_empty() {
                let to_add = std::mem::take(&mut self.pending_add);
                for (fd, src) in to_add {
                    if self.sources.contains_key(&fd) {
                        continue;
                    }
                    if self.backend.register(fd, src.interest).is_ok() {
                        self.sources.insert(fd, src);
                    }
                }
            }
        }
    }

    fn drain_shared_tasks(&mut self) {
        while let Ok(t) = self.shared_rx.try_recv() {
            self.local_tasks.push_back(t);
        }
    }

    fn run_local_tasks(&mut self) {
        while let Some(task) = self.local_tasks.pop_front() {
            task(self);
            if self.exit_requested {
                break;
            }
        }
    }

    fn run_expired_timers(&mut self) {
        let now = Instant::now();
        while let Some(Reverse(t)) = self.timers.peek() {
            if t.when > now {
                break;
            }
            let Reverse(t) = self.timers.pop().unwrap();
            (t.task)(self);
            if self.exit_requested {
                return;
            }
        }
    }

    fn compute_timeout(&self) -> Option<Duration> {
        if !self.local_tasks.is_empty() {
            return Some(Duration::from_millis(0));
        }
        let Some(Reverse(next)) = self.timers.peek() else {
            return None;
        };
        let now = Instant::now();
        Some(next.when.saturating_duration_since(now))
    }
}
