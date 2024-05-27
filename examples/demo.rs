use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};

use hyper::{
    header::CONTENT_TYPE,
    service::service_fn,
    Server,
    {service::make_service_fn, Body, Request, Response},
};
use prometheus_client::{encoding::text::encode, registry::Registry};
use tokio::runtime::Handle;
use tracing::{error, info};

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

fn main() {
    tracing_subscriber::fmt::init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        //let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let runtime_monitor = tokio_metrics_exporter::RuntimeMonitor::new(rt.handle());
    let runtime_collector = tokio_metrics_exporter::RuntimeCollector::new(runtime_monitor);

    let t = std::thread::Builder::new()
        .name("monitor".to_string())
        .spawn(|| {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let mut registry = <Registry>::default();
                registry
                    .sub_registry_with_prefix("tokio")
                    .register_collector(Box::new(runtime_collector));
                server(registry.into()).await;
            });
        })
        .unwrap();

    // rt.block_on(block_runtime());
    // rt.block_on(steal());
    // let handle = Box::leak(Box::new(rt.handle().clone()));
    // rt.block_on(remote_spawn(handle));
    // rt.block_on(local_spawn());
    rt.block_on(local_spawn2());

    t.join().unwrap();
}

#[allow(dead_code)]
async fn block_runtime() {
    for _i in 0..100 {
        tokio::spawn(indice_steal());
    }
}

#[allow(dead_code)]
async fn steal() {
    for _i in 0..10000 {
        tokio::spawn(indice_steal()).await.unwrap();
    }
}

// Good catch!
#[allow(dead_code)]
async fn indice_steal() {
    let (tx, rx) = mpsc::channel();

    tokio::spawn(async move {
        tokio::spawn(async move {
            tx.send(()).unwrap();
        });
        // Spawn a task that bumps the previous task out of the "next scheduled" slot.
        tokio::spawn(async {});
        rx.recv().unwrap();
    })
    .await
    .unwrap();
}

#[allow(dead_code)]
async fn remote_spawn(handle: &'static Handle) {
    std::thread::spawn(move || async move {
        for _ in 0..100000 {
            handle.spawn(async {}).await.unwrap();
        }
    })
    .join()
    .unwrap()
    .await;
}

// used with current_thread
#[allow(dead_code)]
async fn local_spawn() {
    for _ in 0..10000 {
        let task = async {
            tokio::spawn(async {});
            tokio::spawn(async {});
        };
        tokio::spawn(task).await.unwrap();
    }
}

#[allow(dead_code)]
async fn local_spawn2() {
    static SPINLOCK: AtomicBool = AtomicBool::new(true);

    // block the other worker thread
    tokio::spawn(async { while SPINLOCK.load(Ordering::SeqCst) {} });

    // FIXME: why does this need to be in a `spawn`?
    let _ = tokio::spawn(async {
        for _ in 0..100000 {
            tokio::spawn(async {});
        }
    }).await;

    SPINLOCK.store(false, Ordering::SeqCst);
}
