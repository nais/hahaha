#[macro_use]
extern crate lazy_static;

use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    runtime::{events::Reporter, Controller},
    Client,
};
use std::env;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{debug, warn};
use tracing_subscriber::prelude::*;

mod actions;
mod api;
mod pod;
mod prometheus;
mod reconciler;

use crate::prometheus::prometheus_server;

static PROMETHEUS_PORT: u16 = 8999;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let rust_log_env = env::var("RUST_LOG").unwrap_or_else(|_| "hahaha=info,kube=warn".to_string());
    let filter_layer = tracing_subscriber::EnvFilter::builder()
        .with_regex(false)
        .parse_lossy(&rust_log_env);
    let format_layer = tracing_subscriber::fmt::layer().json().flatten_event(true);
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(format_layer)
        .init();

    let actions = actions::generate();
    let client = Client::try_default().await?;

    let pods: Api<Pod> = Api::all(client.clone());
    let lp = ListParams::default().labels("nais.io/naisjob=true");

    let h = hostname::get()?;
    let host_name = h.to_str().unwrap_or("hahaha-1337");

    let reporter = Reporter {
        controller: "hahaha".into(),
        instance: Some(host_name.into()),
    };

    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();
    let prom = tokio::spawn(async move {
        prometheus_server(PROMETHEUS_PORT, shutdown_clone.notified())
            .await
            .unwrap();
    });

    Controller::new(pods, lp)
        .shutdown_on_signal()
        .run(
            reconciler::reconcile,
            reconciler::error_policy,
            Arc::new(reconciler::Data {
                client,
                reporter,
                actions,
            }),
        )
        .for_each(|res| async move {
            match res {
                Ok(o) => debug!("reconciled {}, planned action: {:?}", o.0.name, o.1),
                Err(e) => warn!("reconcile failed: {}", e),
            }
        })
        .await;

    // we're likely not ever reaching down here, but let's be nice about it if we do
    shutdown.notify_one();
    prom.await?;
    Ok(())
}
