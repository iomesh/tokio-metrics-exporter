//! Benchmark implementation details of the threaded scheduler. These benches are
//! intended to be used as a form of regression testing and not as a general
//! purpose benchmark demonstrating real-world performance.

use prometheus_client::registry::Registry;
use tokio::runtime::{self, Runtime};
use tokio::sync::oneshot;

use std::sync::atomic::Ordering::Relaxed;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use criterion::{criterion_group, criterion_main, Criterion};

const NUM_WORKERS: usize = 4;
const NUM_SPAWN: usize = 10_000;
const STALL_DUR: Duration = Duration::from_micros(10);

fn rt_multi_spawn_many_local(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    _name: &str,
) {
    let (rt, registry) = rt();

    let (tx, rx) = mpsc::sync_channel(1000);
    let rem = Arc::new(AtomicUsize::new(0));

    group.bench_function("spawn_many_local", |b| {
        b.iter(|| {
            rem.store(NUM_SPAWN, Relaxed);

            rt.block_on(async {
                for _ in 0..NUM_SPAWN {
                    let tx = tx.clone();
                    let rem = rem.clone();

                    tokio::spawn(async move {
                        if 1 == rem.fetch_sub(1, Relaxed) {
                            tx.send(()).unwrap();
                        }
                    });
                }

                rx.recv().unwrap();
            });

            if let Some(reg) = &registry {
                let _ = reg.downcast_ref::<Registry>();
            }
        })
    });
}

fn rt_multi_spawn_many_remote_idle(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    _name: &str,
) {
    let (rt, registry) = rt();

    let mut handles = Vec::with_capacity(NUM_SPAWN);

    group.bench_function("spawn_many_remote_idle", |b| {
        b.iter(|| {
            for _ in 0..NUM_SPAWN {
                handles.push(rt.spawn(async {}));
            }

            rt.block_on(async {
                for handle in handles.drain(..) {
                    handle.await.unwrap();
                }
            });

            if let Some(reg) = &registry {
                let _ = reg.downcast_ref::<Registry>();
            }
        })
    });
}

// The runtime is busy with tasks that consume CPU time and yield. Yielding is a
// lower notification priority than spawning / regular notification.
fn rt_multi_spawn_many_remote_busy1(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    _name: &str,
) {
    let (rt, registry) = rt();
    let rt_handle = rt.handle();
    let mut handles = Vec::with_capacity(NUM_SPAWN);
    let flag = Arc::new(AtomicBool::new(true));

    // Spawn some tasks to keep the runtimes busy
    for _ in 0..(2 * NUM_WORKERS) {
        let flag = flag.clone();
        rt.spawn(async move {
            while flag.load(Relaxed) {
                tokio::task::yield_now().await;
                stall();
            }
        });
    }

    group.bench_function("spawn_many_remote_busy1", |b| {
        b.iter(|| {
            for _ in 0..NUM_SPAWN {
                handles.push(rt_handle.spawn(async {}));
            }

            rt.block_on(async {
                for handle in handles.drain(..) {
                    handle.await.unwrap();
                }
            });

            if let Some(reg) = &registry {
                let _ = reg.downcast_ref::<Registry>();
            }
        })
    });

    flag.store(false, Relaxed);
}

// The runtime is busy with tasks that consume CPU time and spawn new high-CPU
// tasks. Spawning goes via a higher notification priority than yielding.
fn rt_multi_spawn_many_remote_busy2(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    _name: &str,
) {
    const NUM_SPAWN: usize = 1_000;

    let (rt, registry) = rt();
    let rt_handle = rt.handle();
    let mut handles = Vec::with_capacity(NUM_SPAWN);
    let flag = Arc::new(AtomicBool::new(true));

    // Spawn some tasks to keep the runtimes busy
    for _ in 0..(NUM_WORKERS) {
        let flag = flag.clone();
        fn iter(flag: Arc<AtomicBool>) {
            tokio::spawn(async {
                if flag.load(Relaxed) {
                    stall();
                    iter(flag);
                }
            });
        }
        rt.spawn(async {
            iter(flag);
        });
    }

    group.bench_function("spawn_many_remote_busy2", |b| {
        b.iter(|| {
            for _ in 0..NUM_SPAWN {
                handles.push(rt_handle.spawn(async {}));
            }

            rt.block_on(async {
                for handle in handles.drain(..) {
                    handle.await.unwrap();
                }
            });

            if let Some(reg) = &registry {
                let _ = reg.downcast_ref::<Registry>();
            }
        })
    });

    flag.store(false, Relaxed);
}

fn rt_multi_yield_many(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    _name: &str,
) {
    const NUM_YIELD: usize = 1_000;
    const TASKS: usize = 200;

    group.bench_function("yield_many", |b| {
        let (rt, registry) = rt();
        let (tx, rx) = mpsc::sync_channel(TASKS);

        b.iter(move || {
            for _ in 0..TASKS {
                let tx = tx.clone();

                rt.spawn(async move {
                    for _ in 0..NUM_YIELD {
                        tokio::task::yield_now().await;
                    }

                    tx.send(()).unwrap();
                });
            }

            for _ in 0..TASKS {
                rx.recv().unwrap();
            }

            if let Some(reg) = &registry {
                let _ = reg.downcast_ref::<Registry>();
            }
        })
    });
}

fn rt_multi_ping_pong(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    _name: &str,
) {
    const NUM_PINGS: usize = 1_000;

    let (rt, registry) = rt();

    let (done_tx, done_rx) = mpsc::sync_channel(1000);
    let rem = Arc::new(AtomicUsize::new(0));

    group.bench_function("ping_pong", |b| {
        b.iter(|| {
            let done_tx = done_tx.clone();
            let rem = rem.clone();
            rem.store(NUM_PINGS, Relaxed);

            rt.block_on(async {
                tokio::spawn(async move {
                    for _ in 0..NUM_PINGS {
                        let rem = rem.clone();
                        let done_tx = done_tx.clone();

                        tokio::spawn(async move {
                            let (tx1, rx1) = oneshot::channel();
                            let (tx2, rx2) = oneshot::channel();

                            tokio::spawn(async move {
                                rx1.await.unwrap();
                                tx2.send(()).unwrap();
                            });

                            tx1.send(()).unwrap();
                            rx2.await.unwrap();

                            if 1 == rem.fetch_sub(1, Relaxed) {
                                done_tx.send(()).unwrap();
                            }
                        });
                    }
                });

                done_rx.recv().unwrap();
            });

            if let Some(reg) = &registry {
                let _ = reg.downcast_ref::<Registry>();
            }
        })
    });
}

fn rt_multi_chained_spawn(
    group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>,
    _name: &str,
) {
    const ITER: usize = 1_000;

    fn iter(done_tx: mpsc::SyncSender<()>, n: usize) {
        if n == 0 {
            done_tx.send(()).unwrap();
        } else {
            tokio::spawn(async move {
                iter(done_tx, n - 1);
            });
        }
    }

    group.bench_function("chained_spawn", |b| {
        let (rt, registry) = rt();
        let (done_tx, done_rx) = mpsc::sync_channel(1000);

        b.iter(move || {
            let done_tx = done_tx.clone();

            rt.block_on(async {
                tokio::spawn(async move {
                    iter(done_tx, ITER);
                });

                done_rx.recv().unwrap();
            });

            if let Some(reg) = &registry {
                let _ = reg.downcast_ref::<Registry>();
            }
        })
    });
}

fn rt() -> (Runtime, Option<Box<dyn std::any::Any>>) {
    let mut builder = runtime::Builder::new_multi_thread();
    builder.worker_threads(NUM_WORKERS).enable_all();

    #[cfg(tokio_unstable)]
    {
        use tokio::runtime::UnhandledPanic;
        use tokio_metrics_exporter::{RuntimeCollector, RuntimeMonitor};

        let runtime = builder
            .unhandled_panic(UnhandledPanic::ShutdownRuntime)
            .build()
            .unwrap();
        let monitor = RuntimeMonitor::new(runtime.handle());
        let collector = RuntimeCollector::new(monitor);
        let mut registry = Registry::default();
        registry
            .sub_registry_with_prefix("tokio")
            .register_collector(Box::new(collector));
        (runtime, Some(Box::new(registry)))
    }

    #[cfg(not(tokio_unstable))]
    {
        let runtime = builder.build().unwrap();
        (runtime, None)
    }
}

fn stall() {
    let now = Instant::now();
    while now.elapsed() < STALL_DUR {
        std::thread::yield_now();
    }
}

macro_rules! define_benchmarks {
    ($group:ident, $suffix:literal) => {
        fn $group(c: &mut Criterion) {
            let mut group = c.benchmark_group(concat!("rt_multi_scheduler", $suffix));

            rt_multi_spawn_many_local(&mut group, concat!("spawn_many_local", $suffix));
            rt_multi_spawn_many_remote_idle(&mut group, concat!("spawn_many_remote_idle", $suffix));
            rt_multi_spawn_many_remote_busy1(
                &mut group,
                concat!("spawn_many_remote_busy1", $suffix),
            );
            rt_multi_spawn_many_remote_busy2(
                &mut group,
                concat!("spawn_many_remote_busy2", $suffix),
            );
            rt_multi_ping_pong(&mut group, concat!("ping_pong", $suffix));
            rt_multi_yield_many(&mut group, concat!("yield_many", $suffix));
            rt_multi_chained_spawn(&mut group, concat!("chained_spawn", $suffix));

            group.finish();
        }
    };
}

#[cfg(not(tokio_unstable))]
define_benchmarks!(rt_multi_scheduler_stable, "_stable");
#[cfg(tokio_unstable)]
define_benchmarks!(rt_multi_scheduler_unstable, "_unstable");

#[cfg(not(tokio_unstable))]
criterion_group!(benches, rt_multi_scheduler_stable);
#[cfg(not(tokio_unstable))]
criterion_main!(benches);
#[cfg(tokio_unstable)]
criterion_group!(benches, rt_multi_scheduler_unstable);
#[cfg(tokio_unstable)]
criterion_main!(benches);
