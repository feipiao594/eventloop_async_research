use eventloop_async_research::async_rt::{AsyncQueue, TaskGroup, TcpListener};

use eventloop_async_research::async_rt;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct Client {
    id: u64,
    stream: async_rt::TcpStream,
    outbox: AsyncQueue<String>,
}

enum RoomCommand {
    Join(Arc<Client>),
    Leave(u64),
    Broadcast(String),
}

struct ChatRoom {
    cmds: AsyncQueue<RoomCommand>,
    clients: Mutex<Vec<Arc<Client>>>,
}

impl ChatRoom {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            cmds: AsyncQueue::new(),
            clients: Mutex::new(Vec::new()),
        })
    }

    fn join(&self, client: Arc<Client>) {
        self.cmds.push(RoomCommand::Join(client));
    }

    fn leave(&self, id: u64) {
        self.cmds.push(RoomCommand::Leave(id));
    }

    fn broadcast(&self, msg: String) {
        self.cmds.push(RoomCommand::Broadcast(msg));
    }

    async fn run(self: Arc<Self>) {
        while let Some(cmd) = self.cmds.pop().await {
            match cmd {
                RoomCommand::Join(client) => {
                    let mut clients = self.clients.lock().unwrap();
                    clients.push(client.clone());
                    let msg = format!(
                        "[ChatServer] user#{} joined ({} online)\n",
                        client.id,
                        clients.len()
                    );
                    drop(clients);
                    self.broadcast_impl(msg);
                }
                RoomCommand::Leave(id) => {
                    let mut clients = self.clients.lock().unwrap();
                    clients.retain(|c| c.id != id);
                    let msg = format!("[ChatServer] user#{} left ({} online)\n", id, clients.len());
                    drop(clients);
                    self.broadcast_impl(msg);
                }
                RoomCommand::Broadcast(msg) => {
                    self.broadcast_impl(msg);
                }
            }
        }
    }

    fn broadcast_impl(&self, msg: String) {
        let clients = self.clients.lock().unwrap().clone();
        for c in clients {
            c.outbox.push(msg.clone());
        }
    }
}

async fn writer(client: Arc<Client>) {
    while let Some(msg) = client.outbox.pop().await {
        let _ = client.stream.send_all(msg.as_bytes()).await;
    }
}

async fn chat_session(client: Arc<Client>, room: Arc<ChatRoom>) {
    let exec = async_rt::current_executor();
    let group = TaskGroup::new(exec.clone());
    group.spawn(writer(client.clone()));
    room.join(client.clone());

    let mut buffer = String::new();
    let mut done = false;

    while !done {
        let chunk = match client.stream.recv_some().await {
            Ok(Some(c)) => c,
            Ok(None) => break,
            Err(_) => break,
        };

        buffer.push_str(&String::from_utf8_lossy(&chunk));
        for line in split_lines(&mut buffer) {
            if line == "/quit" {
                let _ = client.stream.send_all(b"[ChatServer] bye\n").await;
                buffer.clear();
                done = true;
                break;
            }
            room.broadcast(format!("user#{}: {}\n", client.id, line));
        }
    }

    room.leave(client.id);
    client.outbox.close();
    group.join().await;
}

fn split_lines(buffer: &mut String) -> Vec<String> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let bytes = buffer.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            let mut end = i;
            if end > start && bytes[end - 1] == b'\r' {
                end -= 1;
            }
            out.push(buffer[start..end].to_string());
            start = i + 1;
        }
    }
    buffer.drain(0..start);
    out
}

#[eventloop_async_research::main]
async fn main() -> std::io::Result<()> {
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[ChatServer] bind {addr} failed: {e}");
            return Ok(());
        }
    };
    eprintln!("[ChatServer] listening on {}", addr);

    let room = ChatRoom::new();
    async_rt::spawn(room.clone().run());

    let next_id = Arc::new(AtomicU64::new(0));

    loop {
        let (stream, peer) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[ChatServer] accept error: {e}");
                continue;
            }
        };
        eprintln!("[ChatServer] accepted {}", peer);

        let id = next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let client = Arc::new(Client {
            id,
            stream,
            outbox: AsyncQueue::new(),
        });

        async_rt::spawn(chat_session(client, room.clone()));
    }
}
