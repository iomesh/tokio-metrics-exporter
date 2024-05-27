#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let handle = tokio::runtime::Handle::current();
    let monitor = tokio_metrics::RuntimeMonitor::new(&handle);
    let mut intervals = monitor.intervals();
    let mut next_interval = || intervals.next().unwrap();

    let interval = next_interval(); // end of interval 1
    assert_eq!(interval.total_park_count, 0);

    induce_parks().await;

    let interval = next_interval();
    println!("{:?}", interval.total_park_count);// end of interval 2
    assert!(interval.total_park_count >= 1); // usually 1 or 2 parks
}

async fn induce_parks() {
    let _ = tokio::time::timeout(std::time::Duration::ZERO, async {
        loop { tokio::task::yield_now().await; }
    }).await;
}
