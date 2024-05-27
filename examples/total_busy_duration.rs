use tokio::time::Duration;

fn main() {
 let start = tokio::time::Instant::now();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let handle = rt.handle();
    let monitor = tokio_metrics::RuntimeMonitor::new(&handle);
    let mut intervals = monitor.intervals();
    let mut next_interval = || intervals.next().unwrap();

    let delay_1s = Duration::from_secs(1);
    let delay_3s = Duration::from_secs(3);

    rt.block_on(async {
        // keep the main task busy for 1s
        spin_for(delay_1s);

        // spawn a task and keep it busy for 2s
        let _ = tokio::spawn(async move {
            spin_for(delay_3s);
        }).await;
    });

    // flush metrics
    drop(rt);

    let elapsed = start.elapsed();

    let interval =  next_interval(); // end of interval 2
    assert!(interval.total_busy_duration >= delay_1s + delay_3s);
    assert!(interval.total_busy_duration <= elapsed);
}

/// Block the current thread for a given `duration`.
fn spin_for(duration: Duration) {
    let start = tokio::time::Instant::now();
    while start.elapsed() <= duration {}
}
