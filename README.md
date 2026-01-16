# eventloop_async_research

一个与 RopUI 项目本身无关的 Rust 调研小项目：实现一个最小 EventLoop（单线程），支持：

- Linux `epoll`
- Unix `poll`（作为通用后端/兜底）

该项目为使用 vibe coding，从 RopUI 的 c++ 代码转换而来

功能点（刻意保持简单）：

- `Handle::post(...)`：跨线程投递任务 + wakeup
- `EventLoop::post_delayed(...)`：定时任务
- `EventLoop::add_io(...)`：注册 fd 的可读/可写回调

运行：

```bash
cd example/eventloop_async_research
cargo run --example linux_loop_core_runner
```

可选：指定后端（默认 Linux 用 `epoll`，其他 Unix 用 `poll`）

```bash
cargo run --example linux_loop_core_runner -- epoll
cargo run --example linux_loop_core_runner -- poll
```