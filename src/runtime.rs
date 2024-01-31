use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeMetric},
    metrics::{counter::Counter, gauge::Gauge},
    registry::Unit,
};
use tokio_metrics::{RuntimeIntervals, RuntimeMonitor};

#[derive(Debug)]
pub struct RuntimeCollector {
    metrics: RuntimeMetrics,
    intervals: Mutex<RuntimeIntervals>,
}

impl RuntimeCollector {
    pub fn new(monitor: RuntimeMonitor) -> Self {
        let intervals = Mutex::new(monitor.intervals());
        let metrics = RuntimeMetrics::default();
        Self { metrics, intervals }
    }
}

impl Collector for RuntimeCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        macro_rules! encode {
            ($name:ident, $description:expr, $unit:expr, $encoder:expr,) => {
                let metric_encoder = $encoder.encode_descriptor(
                    stringify!($name),
                    $description,
                    $unit,
                    self.metrics.$name.metric_type(),
                )?;
                self.metrics.$name.encode(metric_encoder)?;
            };
        }

        let interval = self
            .intervals
            .lock()
            .expect("should be able to lock intervals")
            .next()
            .expect("should always be another interval");

        self.metrics.update(interval);

        encode!(
            workers_count,
            "The number of worker threads used by the runtime",
            None,
            encoder,
        );
        encode!(
            total_park_count,
            "The number of times worker threads parked",
            None,
            encoder,
        );
        encode!(
            max_park_count,
            "The maximum number of times any worker threads parked",
            None,
            encoder,
        );
        encode!(
            min_park_count,
            "The minimum number of times any worker threads parked",
            None,
            encoder,
        );
        encode!(
            mean_poll_duration,
            "The minimum number of times any worker threads parked",
            Some(&Unit::Seconds),
            encoder,
        );
        encode!(
            mean_poll_duration_worker_min,
            "The average duration of a single invocation of poll on a task on the worker with the lowest value.",
            Some(&Unit::Seconds),
            encoder,
        );
        encode!(
            mean_poll_duration_worker_max,
            "The average duration of a single invocation of poll on a task on the worker with the highest value.",
            Some(&Unit::Seconds),
            encoder,
        );
        encode!(
            total_noop_count,
            "The number of times worker threads unparked but performed no work before parking again",
            None,
            encoder,
        );
        encode!(
            max_noop_count,
            "The maximum number of times any worker thread unparked but performed no work before parking again.",
            None,
            encoder,
        );
        encode!(
            min_noop_count,
            "The minimum number of times any worker thread unparked but performed no work before parking again.",
            None,
            encoder,
        );
        encode!(
            total_steal_count,
            "The number of tasks worker threads stole from another worker thread",
            None,
            encoder,
        );
        encode!(
            max_steal_count,
            "The maximum number of tasks any worker thread stole from another worker thread.",
            None,
            encoder,
        );
        encode!(
            min_steal_count,
            "The minimum number of tasks any worker thread stole from another worker thread.",
            None,
            encoder,
        );
        encode!(
            total_steal_operations,
            "The number of times worker threads stole tasks from another worker thread",
            None,
            encoder,
        );
        encode!(
            max_steal_operations,
            "The maximum number of times any worker thread stole tasks from another worker thread.",
            None,
            encoder,
        );
        encode!(
            min_steal_operations,
            "The minimum number of times any worker thread stole tasks from another worker thread.",
            None,
            encoder,
        );
        encode!(
            num_remote_schedules,
            "The number of tasks scheduled from **outside** of the runtime",
            None,
            encoder,
        );
        encode!(
            total_local_schedule_count,
            "The number of tasks scheduled from worker threads",
            None,
            encoder,
        );
        encode!(
            max_local_schedule_count,
            "The maximum number of tasks scheduled from any one worker thread.",
            None,
            encoder,
        );
        encode!(
            min_local_schedule_count,
            "The minimum number of tasks scheduled from any one worker thread.",
            None,
            encoder,
        );
        encode!(
            total_overflow_count,
            "The number of times worker threads saturated their local queues",
            None,
            encoder,
        );
        encode!(
            max_overflow_count,
            "The maximum number of times any one worker saturated its local queue.",
            None,
            encoder,
        );
        encode!(
            min_overflow_count,
            "The minimum number of times any one worker saturated its local queue.",
            None,
            encoder,
        );
        encode!(
            total_polls_count,
            "The number of tasks that have been polled across all worker threads",
            None,
            encoder,
        );
        encode!(
            max_polls_count,
            "The maximum number of tasks that have been polled in any worker thread.",
            None,
            encoder,
        );
        encode!(
            min_polls_count,
            "The minimum number of tasks that have been polled in any worker thread.",
            None,
            encoder,
        );
        encode!(
            total_busy_duration,
            "The amount of time worker threads were busy",
            Some(&Unit::Seconds),
            encoder,
        );
        encode!(
            max_busy_duration,
            "The maximum amount of time a worker thread was busy.",
            Some(&Unit::Seconds),
            encoder,
        );
        encode!(
            min_busy_duration,
            "The minimum amount of time a worker thread was busy.",
            Some(&Unit::Seconds),
            encoder,
        );
        encode!(
            injection_queue_depth,
            "The number of tasks currently scheduled in the runtime's injection queue",
            None,
            encoder,
        );
        encode!(
            total_local_queue_depth,
            "The total number of tasks currently scheduled in workers' local queues",
            None,
            encoder,
        );
        encode!(
            max_local_queue_depth,
            "The maximum number of tasks currently scheduled any worker’s local queue.",
            None,
            encoder,
        );
        encode!(
            min_local_queue_depth,
            "The minimum number of tasks currently scheduled any worker’s local queue.",
            None,
            encoder,
        );
        encode!(
            elapsed,
            "Total amount of time elapsed since observing runtime metrics.",
            Some(&Unit::Seconds),
            encoder,
        );
        encode!(
            budget_forced_yield_count,
            "Returns the number of times that tasks have been forced to yield back to the scheduler after exhausting their task budgets",
            None,
            encoder,
        );
        encode!(
            io_driver_ready_count,
            "Returns the number of ready events processed by the runtime’s I/O driver",
            None,
            encoder,
        );

        Ok(())
    }
}

// Current RuntimeMetrics
// https://docs.rs/tokio-metrics/latest/tokio_metrics/struct.RuntimeMetrics.html
#[derive(Debug, Default)]
struct RuntimeMetrics {
    workers_count: Gauge,
    total_park_count: Counter,
    max_park_count: Gauge,
    min_park_count: Gauge,
    mean_poll_duration: Gauge<f64, AtomicU64>,
    mean_poll_duration_worker_min: Gauge<f64, AtomicU64>,
    mean_poll_duration_worker_max: Gauge<f64, AtomicU64>,
    // poll_count_histogram: Histogram,
    total_noop_count: Counter,
    max_noop_count: Gauge,
    min_noop_count: Gauge,
    total_steal_count: Counter,
    max_steal_count: Gauge,
    min_steal_count: Gauge,
    total_steal_operations: Counter,
    max_steal_operations: Gauge,
    min_steal_operations: Gauge,
    num_remote_schedules: Counter,
    total_local_schedule_count: Counter,
    max_local_schedule_count: Gauge,
    min_local_schedule_count: Gauge,
    total_overflow_count: Counter,
    max_overflow_count: Gauge,
    min_overflow_count: Gauge,
    total_polls_count: Counter,
    max_polls_count: Gauge,
    min_polls_count: Gauge,
    total_busy_duration: Counter<f64, AtomicU64>,
    max_busy_duration: Gauge<f64, AtomicU64>,
    min_busy_duration: Gauge<f64, AtomicU64>,
    injection_queue_depth: Gauge,
    total_local_queue_depth: Gauge,
    max_local_queue_depth: Gauge,
    min_local_queue_depth: Gauge,
    elapsed: Gauge<f64, AtomicU64>,
    budget_forced_yield_count: Counter,
    io_driver_ready_count: Counter,
}

impl RuntimeMetrics {
    fn update(&self, data: tokio_metrics::RuntimeMetrics) {
        macro_rules! inc_by {
            ( $field:ident, "u64" ) => {{
                self.$field.inc_by(data.$field as u64);
            }};
            ( $field:ident, "dur" ) => {{
                self.$field.inc_by(data.$field.as_secs_f64());
            }};
        }
        macro_rules! set_by {
            ( $field:ident, "i64" ) => {{
                self.$field.set(data.$field as i64);
            }};
            ( $field:ident, "dur" ) => {{
                self.$field.set(data.$field.as_secs_f64());
            }};
        }

        set_by!(workers_count, "i64");
        inc_by!(total_park_count, "u64");
        set_by!(max_park_count, "i64");
        set_by!(min_park_count, "i64");
        set_by!(mean_poll_duration, "dur");
        set_by!(mean_poll_duration_worker_min, "dur");
        set_by!(mean_poll_duration_worker_max, "dur");
        // TODO: poll_count_hist
        // self.poll_count_histogram.observe(data.)
        inc_by!(total_noop_count, "u64");
        set_by!(max_noop_count, "i64");
        set_by!(min_noop_count, "i64");
        inc_by!(total_steal_count, "u64");
        set_by!(max_steal_count, "i64");
        set_by!(min_steal_count, "i64");
        inc_by!(total_steal_operations, "u64");
        set_by!(max_steal_operations, "i64");
        set_by!(min_steal_operations, "i64");
        inc_by!(num_remote_schedules, "u64");
        inc_by!(total_local_schedule_count, "u64");
        set_by!(max_local_schedule_count, "i64");
        set_by!(min_local_schedule_count, "i64");
        inc_by!(total_overflow_count, "u64");
        set_by!(max_overflow_count, "i64");
        set_by!(min_overflow_count, "i64");
        inc_by!(total_polls_count, "u64");
        set_by!(max_polls_count, "i64");
        set_by!(min_polls_count, "i64");
        inc_by!(total_busy_duration, "dur");
        set_by!(max_busy_duration, "dur");
        set_by!(min_busy_duration, "dur");
        set_by!(injection_queue_depth, "i64");
        set_by!(total_local_queue_depth, "i64");
        set_by!(max_local_queue_depth, "i64");
        set_by!(min_local_queue_depth, "i64");
        inc_by!(elapsed, "dur");
        inc_by!(budget_forced_yield_count, "u64");
        inc_by!(io_driver_ready_count, "u64");
    }
}
