use futures::Future;
use hyper::service::{make_service_fn, service_fn};
use hyper::{server::Server, Body, Request, Response};
use prometheus::{register_int_counter_vec, Encoder, IntCounterVec, TextEncoder};
use tracing::{error, info};

lazy_static! {
    pub static ref SIDECAR_SHUTDOWNS: IntCounterVec = register_int_counter_vec!(
        "sidecar_shutdowns",
        "Number of sidecar shutdowns",
        &["container", "pod", "namespace"],
    )
    .unwrap();
    pub static ref FAILED_SIDECAR_SHUTDOWNS: IntCounterVec = register_int_counter_vec!(
        "failed_sidecar_shutdowns",
        "Number of failed sidecar shutdowns",
        &["container", "pod", "namespace"],
    )
    .unwrap();
}

async fn metric_service(_req: Request<Body>) -> hyper::Result<Response<Body>> {
    let encoder = TextEncoder::new();
    let mut buffer = vec![];
    let mf = prometheus::gather();
    encoder.encode(&mf, &mut buffer).unwrap();
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, encoder.format_type())
        .body(Body::from(buffer))
        .unwrap())
}

pub async fn prometheus_server<F>(port: u16, shutdown: F) -> hyper::Result<()>
where
    F: Future<Output = ()>,
{
    let addr = ([127, 0, 0, 1], port).into();
    info!("serving prometheus on http://{}", addr);

    let service = make_service_fn(move |_| async { Ok::<_, hyper::Error>(service_fn(metric_service)) });
    let err = Server::bind(&addr)
        .serve(service)
        .with_graceful_shutdown(shutdown)
        .await;
    match &err {
        Ok(()) => info!("stopped prometheus server successfully"),
        Err(e) => error!("error while shutting down: {}", e),
    }
    Ok(())
}


#[tokio::test]
async fn prometheus_server_shuts_down_gracefully() {
    use std::sync::Arc;
    use tokio::sync::Notify;
    
    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();
    let server = tokio::spawn(async move {
        prometheus_server(8999, shutdown_clone.notified())
            .await
            .unwrap();
    });
    shutdown.notify_one();
    let ret = server.await;

    assert!(ret.is_ok())

}
