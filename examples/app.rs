use std::{future::Future, sync::Arc, task::Poll, time::Duration};

use hyper::{
    header::CONTENT_TYPE,
    service::service_fn,
    Server,
    {service::make_service_fn, Body, Request, Response},
};
use prometheus_client::{encoding::text::encode, registry::Registry};
use tracing::{error, info};

static HELP: &str = r#"
Example tokio-metrics-instrumented app

USAGE:
    app [OPTIONS]

OPTIONS:
    -h, help    prints this message
    blocks      Includes a (misbehaving) blocking task
    burn        Includes a (misbehaving) task that spins CPU with self-wakes
    coma        Includes a (misbehaving) task that forgets to register a waker
    noyield     Includes a (misbehaving) task that spawns tasks that never yield
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt::init();

    let handle = tokio::runtime::Handle::current();
    let runtime_monitor = tokio_metrics_exporter::RuntimeMonitor::new(&handle);
    let runtime_collector = tokio_metrics_exporter::RuntimeCollector::new(runtime_monitor);
    let task_collector = tokio_metrics_exporter::global_task_collector();

    tokio::spawn(async move {
        let mut registry = <Registry>::default();
        registry
            .sub_registry_with_prefix("tokio")
            .register_collector(Box::new(runtime_collector));
        registry.register_collector(Box::new(task_collector));
        server(registry.into()).await;
    });

    // spawn optional extras from CLI args
    // skip first which is command name
    for opt in std::env::args().skip(1) {
        let task_monitor = tokio_metrics_exporter::TaskMonitor::new();
        match &*opt {
            "blocks" => {
                task_collector.add("blocks", task_monitor.clone()).unwrap();
                tokio::spawn(task_monitor.instrument(double_sleepy(1, 10)));
            }
            "coma" => {
                task_collector.add("coma", task_monitor.clone()).unwrap();
                tokio::spawn(task_monitor.instrument(std::future::pending::<()>()));
            }
            "burn" => {
                task_collector.add("burn", task_monitor.clone()).unwrap();
                tokio::spawn(task_monitor.instrument(burn(1, 10)));
            }
            "noyield" => {
                task_collector.add("noyield", task_monitor.clone()).unwrap();
                tokio::spawn(task_monitor.instrument(no_yield(20, task_collector)));
            }
            "blocking" => {
                task_collector
                    .add("spawn_blocking", task_monitor.clone())
                    .unwrap();
                tokio::spawn(task_monitor.instrument(spawn_blocking(5)));
            }
            "help" | "-h" => {
                eprintln!("{}", HELP);
                return Ok(());
            }
            wat => {
                return Err(
                    format!("unknown option: {:?}, run with '-h' to see options", wat).into(),
                )
            }
        }
    }

    let task1_monitor = tokio_metrics_exporter::TaskMonitor::new();
    task_collector.add("task1", task1_monitor.clone()).unwrap();
    let task1 = tokio::spawn(task1_monitor.instrument(spawn_tasks(1, 10, task_collector)));
    let task2_monitor = tokio_metrics_exporter::TaskMonitor::new();
    task_collector.add("task2", task2_monitor.clone()).unwrap();
    let task2 = tokio::spawn(task2_monitor.instrument(spawn_tasks(10, 30, task_collector)));

    let result = tokio::try_join! {
        task1,
        task2,
    };
    result?;

    Ok(())
}

async fn spawn_tasks(
    min: u64,
    max: u64,
    task_collector: &'static tokio_metrics_exporter::TaskCollector,
) {
    let mut t = 0;
    loop {
        t += 1;
        for i in min..max {
            let task_monitor = tokio_metrics_exporter::TaskMonitor::new();
            task_collector
                .add(format!("wait{t}_{i}").as_str(), task_monitor.clone())
                .unwrap();
            tracing::trace!(i, "spawning wait task");
            tokio::spawn(task_monitor.instrument(wait(i)));

            let sleep = Duration::from_secs(max) - Duration::from_secs(i);
            tracing::trace!(?sleep, "sleeping...");
            tokio::time::sleep(sleep).await;
        }
    }
}

async fn wait(seconds: u64) {
    tracing::debug!("waiting...");
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    tracing::trace!("done!");
}

async fn double_sleepy(min: u64, max: u64) {
    loop {
        for i in min..max {
            // woops!
            std::thread::sleep(Duration::from_secs(i));
            tokio::time::sleep(Duration::from_secs(max - i)).await;
        }
    }
}

async fn burn(min: u64, max: u64) {
    loop {
        for i in min..max {
            for _ in 0..i {
                self_wake().await;
            }
            tokio::time::sleep(Duration::from_secs(i - min)).await;
        }
    }
}

async fn no_yield(seconds: u64, task_collector: &'static tokio_metrics_exporter::TaskCollector) {
    loop {
        let task_monitor = tokio_metrics_exporter::TaskMonitor::new();
        task_collector.add("greedy", task_monitor.clone()).unwrap();
        let handle = tokio::spawn(task_monitor.instrument(async move {
            std::thread::sleep(Duration::from_secs(seconds));
        }));
        _ = handle.await;
    }
}

async fn spawn_blocking(seconds: u64) {
    loop {
        _ = tokio::task::spawn_blocking(move || {
            std::thread::sleep(Duration::from_secs(seconds));
        })
        .await;
    }
}

fn self_wake() -> impl Future<Output = ()> {
    struct SelfWake {
        yielded: bool,
    }

    impl Future for SelfWake {
        type Output = ();

        fn poll(
            mut self: std::pin::Pin<&mut Self>,
            cx: &mut std::task::Context<'_>,
        ) -> Poll<Self::Output> {
            if self.yielded {
                return Poll::Ready(());
            }

            self.yielded = true;
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    SelfWake { yielded: false }
}

async fn serve_req(
    registry: Arc<Registry>,
    _req: Request<Body>,
) -> std::result::Result<Response<Body>, hyper::Error> {
    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    let response = Response::builder()
        .status(200)
        .header(CONTENT_TYPE, "text/html")
        .body(Body::from(buffer))
        .unwrap();

    Ok(response)
}

async fn server(registry: Arc<Registry>) {
    let addr = std::env::var("PROMETHEUS_BIND")
        .unwrap_or("0.0.0.0:20033".to_string())
        .parse()
        .unwrap();
    info!("Prometheus server listening on http://{}", addr);

    let make_svc = make_service_fn(move |_| {
        let registry = registry.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let registry = registry.clone();
                async move { serve_req(registry, req).await }
            }))
        }
    });

    let serve_fut = Server::bind(&addr).serve(make_svc);
    if let Err(err) = serve_fut.await {
        error!("Start Prometheus server failed: {:?}", err);
    }
}
