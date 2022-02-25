use std::{collections::BTreeMap, sync::Arc, time::Duration};

use k8s_openapi::api::core::v1::Pod;
use kube::{
    runtime::{
        controller::{Context, ReconcilerAction},
        events::Reporter,
    },
    Api, Client, Resource, ResourceExt,
};
use thiserror::Error;
use tracing::{debug, warn};

use crate::{actions::Action, api::Destroyer, events::Recorder, pod::Sidecars, prometheus::*};

static REQUEUE_SECONDS: u64 = 300;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}: missing defined action: {1}")]
    MissingDefinedAction(String, String),
    #[error("{0}: could not get job name: {1}")]
    MissingJobName(String, anyhow::Error),
    #[error("{0}: could not shut down sidecar {1}: {2}")]
    SidecarShutdownFailed(String, String, anyhow::Error),
}

pub struct Data {
    client: Client,
    reporter: Reporter,
    actions: BTreeMap<String, Action>,
}

impl Data {
    pub fn new(client: Client, reporter: Reporter, actions: BTreeMap<String, Action>) -> Data {
        Data {
            client,
            reporter,
            actions,
        }
    }
}

pub async fn reconcile(pod: Arc<Pod>, ctx: Context<Data>) -> Result<ReconcilerAction, Error> {
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

pub fn error_policy(_error: &Error, _ctx: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(30)),
    }
}
