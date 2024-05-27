mod task;

pub use task::global_collector as global_task_collector;
pub use task::TaskCollector;
pub use tokio_metrics::TaskMonitor;

macro_rules! cfg_rt {
    ($($item:item)*) => {
        $(
            #[cfg(all(tokio_unstable, feature = "rt"))]
            $item
        )*
    };
}

cfg_rt! {
    mod runtime;
    pub use runtime::RuntimeCollector;
    pub use tokio_metrics::RuntimeMonitor;
}
