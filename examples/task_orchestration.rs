use std::io;
use std::time::Duration;
use eventloop_async_research::async_rt;

#[eventloop_async_research::main]
async fn main() -> io::Result<()> {
    // join_all：等全部完成 + 收集结果（保持原顺序）
    let tasks = vec![
        async_rt::spawn(task_ret("1", 1)),
        async_rt::spawn(task_ret("2", 2)),
        async_rt::spawn(task_ret("3", 1)),
    ];
    let results = eventloop_async_research::async_rt::join_all(tasks).await;
    println!("所有任务完成，结果：{:?}", results);

    // select_any：任一任务完成即处理（可选择 abort 其他任务）
    let handles = vec![
        async_rt::spawn(task_ret("A", 2)),
        async_rt::spawn(task_ret("B", 1)),
    ];

    let mut handles = handles;
    let mut finished = 0usize;
    while !handles.is_empty() {
        let sel = async_rt::select_any(handles).await;
        finished += 1;
        println!(
            "第{}个完成：原始index={}，结果：{:?}",
            finished, sel.index, sel.result
        );
        handles = sel.remaining;
    }

    Ok(())
}

async fn task(name: &str, delay_secs: u64) {
    async_rt::sleep(Duration::from_secs(delay_secs)).await;
    println!("任务{name}完成");
}

async fn task_ret(name: &'static str, delay_secs: u64) -> String {
    task(name, delay_secs).await;
    name.to_string()
}
