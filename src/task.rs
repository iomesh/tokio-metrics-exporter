use std::{
    collections::HashMap,
    fmt::Debug,
    io::{Error, ErrorKind},
    sync::RwLock,
};

use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeMetric},
    metrics::{counter::Counter, family::Family, gauge::Gauge, MetricType},
    registry::Unit,
};
use tokio_metrics::TaskMonitor;

pub struct TaskCollector {
    metrics: TaskMetrics,
    intervals:
        RwLock<HashMap<String, Box<dyn Iterator<Item = tokio_metrics::TaskMetrics> + Send + Sync>>>,
}

impl Debug for TaskCollector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskCollector")
            .field("metrics", &self.metrics)
            .finish()
    }
}

impl TaskCollector {
    pub fn new() -> Self {
        let metrics = TaskMetrics::default();
        let intervals = RwLock::new(HashMap::new());
        Self { metrics, intervals }
    }

    pub fn add(&self, label: &str, monitor: TaskMonitor) -> Result<(), Error> {
        if self.intervals.read().unwrap().contains_key(label) {
            return Err(Error::new(
                ErrorKind::AlreadyExists,
                format!("label {label} already exists"),
            ));
        }
        self.intervals
            .write()
            .unwrap()
            .insert(label.to_string(), Box::new(monitor.intervals()));

        Ok(())
    }

    pub fn remove(&mut self, label: &str) {
        self.intervals.write().unwrap().remove(label);
    }

    pub fn get(&self, label: &str) -> tokio_metrics::TaskMetrics {
        let data = self
            .intervals
            .write()
            .unwrap()
            .get_mut(label)
            .unwrap()
            .next();
        data.unwrap()
    }
}

impl Default for TaskCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Collector for TaskCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), std::fmt::Error> {
        macro_rules! encode {
            ($name:ident, $description:expr, $unit:expr, $encoder:expr, $type:expr, $labels:expr,) => {
                let mut _metric_encoder =
                    $encoder.encode_descriptor(stringify!($name), $description, $unit, $type)?;
                for label in $labels {
                    if let Some(metrics) = self.metrics.$name.get(label) {
                        metrics.encode(
                            _metric_encoder
                                .encode_family(&[("comm".to_owned(), label.to_string())])?,
                        )?;
                    }
                }
            };
        }

        let mut labels = vec![];

        {
            let intervals = self.intervals.read().unwrap();

            for (label, _) in intervals.iter() {
                labels.push(label.to_string());
            }
        }

        for label in &labels {
            let interval = self.get(label);
            self.metrics.update(label, interval);
        }

        encode!(
            instrumented_count,
            "The number of tasks instrumented.",
            None,
            encoder,
            MetricType::Gauge,
            &labels,
        );
        encode!(
            dropped_count,
            "The number of tasks dropped.",
            None,
            encoder,
            MetricType::Gauge,
            &labels,
        );
        encode!(
            first_poll_count,
            "The number of tasks polled for the first time.",
            None,
            encoder,
            MetricType::Gauge,
            &labels,
        );
        encode!(
            total_first_poll_delay,
            "The total duration elapsed between the instant tasks are instrumented, and the instant they are first polled.",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_idled_count,
            "The total number of times that tasks idled, waiting to be awoken.",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_idle_duration,
            "The total duration that tasks idled.",
            Some(&Unit::Seconds),
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_scheduled_count,
            "The total number of times that tasks were awoken (and then, presumably, scheduled for execution).",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_scheduled_duration,
            "The total duration that tasks spent waiting to be polled after awakening.",
            Some(&Unit::Seconds),
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_poll_count,
            "The total number of times that tasks were polled.",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_poll_duration,
            "The total duration elapsed during polls.",
            Some(&Unit::Seconds),
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_fast_poll_count,
            "The total number of times that polling tasks completed swiftly.",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_fast_poll_duration,
            "The total duration of fast polls.",
            Some(&Unit::Seconds),
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_slow_poll_count,
            "The total number of times that polling tasks completed slowly.",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_slow_poll_duration,
            "The total duration of slow polls.",
            Some(&Unit::Seconds),
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_short_delay_count,
            "The total count of tasks with short scheduling delays.",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_long_delay_count,
            "The total count of tasks with long scheduling delays.",
            None,
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_short_delay_duration,
            "The total duration of tasks with short scheduling delays.",
            Some(&Unit::Seconds),
            encoder,
            MetricType::Counter,
            &labels,
        );
        encode!(
            total_long_delay_duration,
            "The total number of times that a task had a long scheduling duration.",
            Some(&Unit::Seconds),
            encoder,
            MetricType::Counter,
            &labels,
        );
        Ok(())
    }
}

// Current RuntimeMetrics
// https://docs.rs/tokio-metrics/latest/tokio_metrics/struct.TaskMetrics.html
#[derive(Debug, Default)]
struct TaskMetrics {
    instrumented_count: Family<String, Gauge>,
    dropped_count: Family<String, Gauge>,
    first_poll_count: Family<String, Gauge>,
    total_first_poll_delay: Family<String, Counter<f64>>,
    total_idled_count: Family<String, Counter>,
    total_idle_duration: Family<String, Counter<f64>>,
    total_scheduled_count: Family<String, Counter>,
    total_scheduled_duration: Family<String, Counter<f64>>,
    total_poll_count: Family<String, Counter>,
    total_poll_duration: Family<String, Counter<f64>>,
    total_fast_poll_count: Family<String, Counter>,
    total_fast_poll_duration: Family<String, Counter<f64>>,
    total_slow_poll_count: Family<String, Counter>,
    total_slow_poll_duration: Family<String, Counter<f64>>,
    total_short_delay_count: Family<String, Counter>,
    total_long_delay_count: Family<String, Counter>,
    total_short_delay_duration: Family<String, Counter<f64>>,
    total_long_delay_duration: Family<String, Counter<f64>>,
}

impl TaskMetrics {
    fn update(&self, label: &String, data: tokio_metrics::TaskMetrics) {
        macro_rules! inc_by {
            ( $field:ident, "u64" ) => {{
                self.$field.get_or_create(label).inc_by(data.$field as u64);
            }};
            ( $field:ident, "dur" ) => {{
                self.$field
                    .get_or_create(label)
                    .inc_by(data.$field.as_secs_f64());
            }};
        }
        macro_rules! set_by {
            ( $field:ident, "i64" ) => {{
                self.$field.get_or_create(label).set(data.$field as i64);
            }};
            ( $field:ident, "dur" ) => {{
                self.$field
                    .get_or_create(label)
                    .set(data.$field.as_secs_f64());
            }};
        }

        set_by!(instrumented_count, "i64");
        set_by!(dropped_count, "i64");
        set_by!(first_poll_count, "i64");

        inc_by!(total_first_poll_delay, "dur");
        inc_by!(total_idled_count, "u64");
        inc_by!(total_idle_duration, "dur");
        inc_by!(total_scheduled_count, "u64");
        inc_by!(total_scheduled_duration, "dur");
        inc_by!(total_poll_count, "u64");
        inc_by!(total_poll_duration, "dur");
        inc_by!(total_fast_poll_count, "u64");
        inc_by!(total_fast_poll_duration, "dur");
        inc_by!(total_slow_poll_count, "u64");
        inc_by!(total_slow_poll_duration, "dur");
        inc_by!(total_short_delay_count, "u64");
        inc_by!(total_long_delay_count, "u64");
        inc_by!(total_short_delay_duration, "dur");
        inc_by!(total_long_delay_duration, "dur");
    }
}
