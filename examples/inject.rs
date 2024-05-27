use std::{future::Future, sync::Arc, task::Poll, time::Duration};

use hyper::{
    header::CONTENT_TYPE,
    service::service_fn,
    Server,
    {service::make_service_fn, Body, Request, Response},
};
use prometheus_client::{encoding::text::encode, registry::Registry};
use tokio::task::spawn_blocking;
use tracing::{error, info};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
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

    /*
        spawn_blocking(move || {
            handle.block_on(async move {
                let mut registry = <Registry>::default();
                registry
                    .sub_registry_with_prefix("tokio")
                    .register_collector(Box::new(runtime_collector));
                registry.register_collector(Box::new(task_collector));
                server(registry.into()).await;
            });
        });
    */
    for _ in 0..10000000 {
        tokio::spawn(steal());
    }

    // for i in 1..=4 {
    //     let task_monitor = tokio_metrics_exporter::TaskMonitor::new();
    //     task_collector
    //         .add(format!("burn{i}").as_str(), task_monitor.clone())
    //         .unwrap();
    //     tokio::spawn(task_monitor.instrument(burn()));
    // }

    std::future::pending::<()>().await;
    Ok(())
}

async fn burn() {
    tokio::time::sleep(Duration::from_millis(1)).await;
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

async fn steal() {
    let (tx, rx) = std::sync::mpsc::channel();

    tokio::spawn(async move {
        tokio::spawn(async move {
            tx.send(()).unwrap();
        });
        // Spawn a task that bumps the previous task out of the "next scheduled" slot.
        tokio::spawn(async {});
        rx.recv().unwrap();
        let _ = tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    })
    .await
    .unwrap();
}
