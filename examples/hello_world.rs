use std::{io, time::Duration};
use eventloop_async_research::async_rt;

#[eventloop_async_research::main]
async fn main() -> io::Result<()> {
    let task1 = async_rt::spawn(async {
        async_rt::sleep(Duration::from_secs(1)).await;
        println!("task1(1s) finished");
    });

    let task2 = async_rt::spawn(async {
        async_rt::sleep(Duration::from_secs(3)).await;
        println!("task2(3s) finished");
    });

    task2.await.unwrap();
    println!("task2.await and task1.await gap");
    task1.await.unwrap();

    Ok(())
}