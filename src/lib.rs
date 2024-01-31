mod task;

pub use task::TaskCollector;

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
}
