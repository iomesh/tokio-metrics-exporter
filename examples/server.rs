use std::{sync::Arc, time::Duration};

use hyper::{
    header::CONTENT_TYPE,
    service::service_fn,
    Server,
    {service::make_service_fn, Body, Request, Response},
};
use prometheus_client::{encoding::text::encode, registry::Registry};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let handle = tokio::runtime::Handle::current();
    let runtime_monitor = tokio_metrics::RuntimeMonitor::new(&handle);
    let runtime_collector = tokio_metrics_exporter::RuntimeCollector::new(runtime_monitor);

    let task_monitor = tokio_metrics::TaskMonitor::new();
    let task_collector = tokio_metrics_exporter::TaskCollector::new();
    let worker1_monitor = task_monitor.clone();
    task_collector
        .add("do_work1", worker1_monitor.clone())
        .unwrap();
    let worker2_monitor = task_monitor.clone();
    task_collector
        .add("do_work2", worker2_monitor.clone())
        .unwrap();
    let worker3_monitor = task_monitor;
    task_collector
        .add("do_work3", worker3_monitor.clone())
        .unwrap();

    tokio::spawn(async move {
        let mut registry = <Registry>::default();
        registry
            .sub_registry_with_prefix("tokio")
            .register_collector(Box::new(runtime_collector));
        registry.register_collector(Box::new(task_collector));
        server(registry.into()).await;
    });

    tokio::join![
        worker1_monitor.instrument(do_work()),
        worker2_monitor.instrument(do_work()),
        worker3_monitor.instrument(do_work())
    ];
}

async fn do_work() {
    loop {
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
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
        .unwrap_or("127.0.0.1:20033".to_string())
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
