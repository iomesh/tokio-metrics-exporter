#[tokio::main(flavor = "current_thread")]
async fn main() {
    let handle = tokio::runtime::Handle::current();
    let monitor = tokio_metrics::RuntimeMonitor::new(&handle);
    let mut intervals = monitor.intervals();
    let mut next_interval = || intervals.next().unwrap();

    assert_eq!(next_interval().total_park_count, 0);

    async {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }.await;

    assert!(next_interval().total_park_count > 0);
}
