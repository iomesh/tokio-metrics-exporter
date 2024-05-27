use tokio::runtime::HistogramScale;
use std::time::Duration;

fn main() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_metrics_poll_count_histogram()
        .metrics_poll_count_histogram_scale(HistogramScale::Linear)
        .metrics_poll_count_histogram_resolution(Duration::from_micros(50))
        .metrics_poll_count_histogram_buckets(12)
        .build()
        .unwrap();

    rt.block_on(async {
        let handle = tokio::runtime::Handle::current();
        let monitor = tokio_metrics::RuntimeMonitor::new(&handle);
        let mut intervals = monitor.intervals();
        let mut next_interval = || intervals.next().unwrap();

        let interval = next_interval();
        println!("poll count histogram {:?}", interval.poll_count_histogram);
    });
}
