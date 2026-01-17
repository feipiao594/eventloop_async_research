use std::io;
use std::time::Duration;

#[eventloop_async_research::main]
async fn main() -> io::Result<()> {
    // join_all：等全部完成 + 收集结果（保持原顺序）
    let tasks = vec![
        eventloop_async_research::async_rt::spawn(async {
            task("1", 1).await;
            "1".to_string()
        }),
        eventloop_async_research::async_rt::spawn(async {
            task("2", 2).await;
            "2".to_string()
        }),
        eventloop_async_research::async_rt::spawn(async {
            task("3", 1).await;
            "3".to_string()
        }),
    ];
    let results = eventloop_async_research::async_rt::join_all(tasks).await;
    println!("所有任务完成，结果：{:?}", results);

    // select_any：任一任务完成即处理（可选择 abort 其他任务）
    let handles = vec![
        eventloop_async_research::async_rt::spawn(async {
            task("A", 2).await;
            "A".to_string()
        }),
        eventloop_async_research::async_rt::spawn(async {
            task("B", 1).await;
            "B".to_string()
        }),
    ];

    let mut handles = handles;
    let mut finished = 0usize;
    while !handles.is_empty() {
        let sel = eventloop_async_research::async_rt::select_any(handles).await;
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
    eventloop_async_research::async_rt::sleep(Duration::from_secs(delay_secs)).await;
    println!("任务{name}完成");
}
