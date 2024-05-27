#[tokio::main(flavor = "multi_thread", worker_threads=2)]
async fn main() {
    let handle = tokio::runtime::Handle::current();
    let monitor = tokio_metrics::RuntimeMonitor::new(&handle);
    let mut intervals = monitor.intervals();
    let mut next_interval = || intervals.next().unwrap();

    let interval = next_interval();
    assert_eq!(interval.total_steal_count, 0);
    assert_eq!(interval.min_steal_count, 0);
    assert_eq!(interval.max_steal_count, 0);

    async {
        let (tx, rx) = std::sync::mpsc::channel();

        tokio::spawn(async move {
            tokio::spawn(async move { tx.send(()).unwrap(); });
            // Spawn a task that bumps the previous task out of the "next scheduled" slot.
            tokio::spawn(async {});
            rx.recv().unwrap();
        }).await.unwrap();

        flush_metrics().await;
    }.await;

    let interval = { flush_metrics().await; next_interval() };
    println!("total={}; min={}, max={}", interval.total_steal_count, interval.min_steal_count, interval.max_steal_count);

    let interval = { flush_metrics().await; next_interval() };
    println!("total={}; min={}, max={}", interval.total_steal_count, interval.min_steal_count, interval.max_steal_count);
}

async fn flush_metrics() {
    let _ = tokio::time::sleep(std::time::Duration::ZERO).await;
}
