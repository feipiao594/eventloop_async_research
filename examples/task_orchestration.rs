use std::io;
use std::time::Duration;
use eventloop_async_research::async_rt;

async fn task(name: &str, delay_secs: u64) -> String {
    async_rt::sleep(
        Duration::from_secs(delay_secs)).await;
    println!("任务{name}完成");
    name.to_string()
}

#[eventloop_async_research::main]
async fn main() -> io::Result<()> {
    let tasks = vec![
        async_rt::spawn(task("1", 1)),
        async_rt::spawn(task("2", 2)),
        async_rt::spawn(task("3", 1)),
    ];
    let results = async_rt::join_all(tasks).await;
    println!("所有任务完成，结果：{:?}", results);
    
    let handles = vec![
        async_rt::spawn(task("A", 2)),
        async_rt::spawn(task("B", 1)),
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