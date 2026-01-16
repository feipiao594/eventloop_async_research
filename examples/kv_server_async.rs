use eventloop_async_research::async_rt::{TcpListener, TcpStream};

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

type Store = Arc<Mutex<HashMap<String, String>>>;

#[eventloop_async_research::main]
async fn main() -> io::Result<()> {
    let addr: SocketAddr = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:7070".to_string())
        .parse()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

    let listener = TcpListener::bind(addr)?;
    eprintln!("kv server listening on {addr}");
    eprintln!("usage: cargo run --example kv_server_async -- [poll|epoll] [addr]");

    let store: Store = Arc::new(Mutex::new(HashMap::new()));
    let next_id = Arc::new(AtomicU64::new(0));

    loop {
        let (stream, peer) = listener.accept().await?;
        let store = store.clone();
        let id = next_id.fetch_add(1, Ordering::Relaxed) + 1;
        eventloop_async_research::async_rt::spawn(async move {
            eprintln!("[conn#{id}] {peer} connected");
            if let Err(e) = handle_conn(stream, store).await {
                eprintln!("[conn#{id}] error: {e}");
            }
            eprintln!("[conn#{id}] closed");
        });
    }
}

async fn handle_conn(stream: TcpStream, store: Store) -> io::Result<()> {
    send_line(
        &stream,
        "OK kv-server; commands: GET SET DEL KEYS QUIT HELP",
    )
    .await?;

    let mut buffer = String::new();
    loop {
        let chunk = match stream.recv_some().await? {
            Some(c) => c,
            None => return Ok(()),
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        for line in split_lines(&mut buffer) {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let mut parts = line.splitn(3, ' ');
            let cmd = parts.next().unwrap_or("").to_ascii_uppercase();
            match cmd.as_str() {
                "HELP" => {
                    send_line(&stream, "GET <k> | SET <k> <v> | DEL <k> | KEYS | QUIT").await?;
                }
                "QUIT" => {
                    send_line(&stream, "BYE").await?;
                    return Ok(());
                }
                "GET" => {
                    let Some(key) = parts.next() else {
                        send_line(&stream, "ERR missing key").await?;
                        continue;
                    };
                    let val = store.lock().unwrap().get(key).cloned();
                    match val {
                        Some(v) => send_line(&stream, &format!("VAL {v}")).await?,
                        None => send_line(&stream, "NIL").await?,
                    }
                }
                "SET" => {
                    let Some(key) = parts.next() else {
                        send_line(&stream, "ERR missing key").await?;
                        continue;
                    };
                    let Some(val) = parts.next() else {
                        send_line(&stream, "ERR missing value").await?;
                        continue;
                    };
                    store
                        .lock()
                        .unwrap()
                        .insert(key.to_string(), val.to_string());
                    send_line(&stream, "OK").await?;
                }
                "DEL" => {
                    let Some(key) = parts.next() else {
                        send_line(&stream, "ERR missing key").await?;
                        continue;
                    };
                    let removed = store.lock().unwrap().remove(key).is_some();
                    if removed {
                        send_line(&stream, "OK").await?;
                    } else {
                        send_line(&stream, "NIL").await?;
                    }
                }
                "KEYS" => {
                    let keys: Vec<String> = store.lock().unwrap().keys().cloned().collect();
                    if keys.is_empty() {
                        send_line(&stream, "EMPTY").await?;
                    } else {
                        send_line(&stream, &format!("KEYS {}", keys.join(" "))).await?;
                    }
                }
                _ => {
                    send_line(&stream, "ERR unknown command").await?;
                }
            }
        }
    }
}

async fn send_line(stream: &TcpStream, s: &str) -> io::Result<()> {
    stream.send_all(format!("{s}\n").as_bytes()).await
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
