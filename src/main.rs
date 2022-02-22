#[macro_use]
extern crate lazy_static;

use actions::Action;
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    runtime::{
        controller::{Context, ReconcilerAction},
        events::Reporter,
        Controller,
    },
    Client, Resource, ResourceExt,
};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use thiserror::Error;
use tokio::sync::Notify;
use tracing::{debug, error, warn};
use tracing_subscriber::prelude::*;

mod actions;
mod api;
mod events;
mod pod;
mod prometheus;

use crate::{api::Destroyer, events::Recorder, pod::Sidecars, prometheus::*};

static REQUEUE_SECONDS: u64 = 300;
static PROMETHEUS_PORT: u16 = 8999;

#[derive(Debug, Error)]
enum Error {
    #[error("{0}: missing defined action: {1}")]
    MissingDefinedAction(String, String),
    #[error("{0}: could not get job name: {1}")]
    MissingJobName(String, anyhow::Error),
    #[error("{0}: could not shut down sidecar {1}: {2}")]
    SidecarShutdownFailed(String, String, anyhow::Error),
}

struct Data {
    client: Client,
    reporter: Reporter,
    actions: BTreeMap<String, Action>,
}

async fn reconcile(pod: Arc<Pod>, ctx: Context<Data>) -> Result<ReconcilerAction, Error> {
    let pod_name = pod.name();
    let namespace = match pod.namespace() {
        Some(namespace) => namespace,
        None => "default".into(),
    };

    let running_sidecars = pod.sidecars().unwrap_or_else(|err| {
        warn!("{pod_name}: getting running sidecars: {err}");
        Vec::new()
    });
    if running_sidecars.is_empty() {
        // Move onto the next iteration if there's nothing to look at
        return Ok(ReconcilerAction {
            requeue_after: Some(Duration::from_secs(REQUEUE_SECONDS)),
        });
    }

    // we need a namespaced api to `exec` and `portforward` into the target pod.
    let api: Api<Pod> = Api::namespaced(ctx.get_ref().client.clone(), &namespace);

    // set up a recorder for publishing events to the Pod
    let recorder = Recorder::new(
        ctx.get_ref().client.clone(),
        ctx.get_ref().reporter.clone(),
        pod.object_ref(&()),
    );

    debug!("{pod_name}: needs help shutting down some residual containers");

    let job_name = match pod.job_name() {
        Ok(name) => name,
        Err(e) => return Err(Error::MissingJobName(pod_name, e)),
    };

    for sidecar in running_sidecars {
        let sidecar_name = sidecar.name;
        debug!("{pod_name}: found sidecar {sidecar_name}");
        let action = match ctx.get_ref().actions.get(&sidecar_name) {
            Some(action) => action,
            None => return Err(Error::MissingDefinedAction(pod_name, sidecar_name)),
        };
        let res = api.shutdown(action, &pod_name, &sidecar_name).await;
        if let Err(err) = res {
            if let Err(e) = recorder
                .warn(format!("Unsuccessfully shut down container {sidecar_name}: {err}"))
                .await
            {
                warn!("{pod_name}: couldn't publish Kubernetes Event: {e}");
                TOTAL_UNSUCCESSFUL_EVENT_POSTS.inc();
            }
            FAILED_SIDECAR_SHUTDOWNS
                .with_label_values(&[&sidecar_name, &job_name, &namespace])
                .inc();
            return Err(Error::SidecarShutdownFailed(pod_name, sidecar_name, err));
        }
        if let Err(e) = recorder.info(format!("Shut down container {sidecar_name}")).await {
            warn!("{pod_name}: couldn't publish Kubernetes Event: {e}");
            TOTAL_UNSUCCESSFUL_EVENT_POSTS.inc();
        }
        SIDECAR_SHUTDOWNS
            .with_label_values(&[&sidecar_name, &job_name, &namespace])
            .inc();
    }

    Ok(ReconcilerAction {
        requeue_after: Some(Duration::from_secs(REQUEUE_SECONDS)),
    })
}

fn error_policy(_error: &Error, _ctx: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(30)),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let filter_layer = tracing_subscriber::EnvFilter::from_default_env();
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
    let host_name = match h.to_str() {
        Some(s) => s,
        None => "hahaha-1337", // consider dying here, this should never happen after all.
    };

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
            reconcile,
            error_policy,
            Context::new(Data {
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
