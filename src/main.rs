#[macro_use]
extern crate lazy_static;

use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    runtime::{events::Reporter, utils::try_flatten_applied, watcher},
    Client, Resource, ResourceExt,
};
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{error, info, warn};

mod actions;
mod api;
mod events;
mod pod;
mod prometheus;

use crate::{
    events::Recorder,
    prometheus::*,
    api::Destroyer,
    pod::Sidecars,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=warn");
    tracing_subscriber::fmt::init();

    let actions = actions::generate();
    let client = Client::try_default().await?;

    let pods: Api<Pod> = Api::all(client.clone());
    let lp = ListParams::default().timeout(30).labels("nais.io/ginuudan=enabled");
    let reporter = Reporter {
        controller: "hahaha".into(),
        instance: Some("hahaha-1234".into()), // TODO get instance from cluster when deployed
    };

    let mut ew = try_flatten_applied(watcher(pods, lp)).boxed();

    let shutdown = Arc::new(Notify::new());
    let shutdown_clone = shutdown.clone();
    let prom = tokio::spawn(async move {
        prometheus_server(8999, shutdown_clone.notified()).await.unwrap();
    });

    while let Some(pod) = ew.try_next().await? {
        let pod_name = pod.name();

        let running_sidecars = pod.sidecars().unwrap_or_else(|err| {
            info!("Getting running sidecars for {pod_name}: {err}");
            Vec::new()
        });
        if running_sidecars.is_empty() {
            // Move onto the next iteration if there's nothing to look at
            continue;
        }

        let namespace = match pod.namespace() {
            Some(namespace) => namespace,
            None => "default".into(),
        };
        // we need a namespaced api to `exec` and `portforward` into the target pod.
        let api: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        // set up a recorder for publishing events to the Pod
        let recorder = Recorder::new(client.clone(), reporter.clone(), pod.object_ref(&()));

        info!("{pod_name} in namespace {namespace} needs help shutting down some residual containers!");

        let job_name = match pod.job_name() {
            Ok(name) => name,
            Err(e) => {
                warn!("Getting job name from pod: {e}");
                continue;
            }
        };

        for sidecar in running_sidecars {
            let sidecar_name = sidecar.name;
            let action = match actions.get(&sidecar_name) {
                Some(action) => action,
                None => {
                    warn!("I don't know how to shut down {sidecar_name} (in {pod_name} in namespace {namespace})");
                    continue;
                }
            };
            let res = api.shutdown(action, &pod_name, &sidecar_name).await;
            if let Err(err) = res {
                error!("Couldn't shutdown: {err}");
                recorder
                    .warn(format!("Unsuccessfully shut down container {sidecar_name}"))
                    .await?;
                FAILED_SIDECAR_SHUTDOWNS
                    .with_label_values(&[&sidecar_name, &job_name, &namespace])
                    .inc();
                continue;
            }
            recorder
                .info(format!("Successfully shut down container {sidecar_name}"))
                .await?;
            SIDECAR_SHUTDOWNS
                .with_label_values(&[&sidecar_name, &job_name, &namespace])
                .inc();
            TOTAL_SIDECAR_SHUTDOWNS.inc();
        }
    }

    // we're likely not ever reaching down here, but let's be nice about it if we do
    shutdown.notify_one();
    prom.await?;
    Ok(())
}
