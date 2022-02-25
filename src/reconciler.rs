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

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}: could not shut down sidecar {1}: {2}")]
    SidecarShutdownFailed(String, String, anyhow::Error),
    #[error("{0}: could not get running sidecars: {1}")]
    RunningSidecarError(String, anyhow::Error),
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

    let running_sidecars = match pod.sidecars() {
        Ok(sidecars) => sidecars,
        Err(err) => return Err(Error::RunningSidecarError(pod_name, err)),
    };

    if running_sidecars.is_empty() {
        // There's no need to ever look at this pod again if there are no running sidecars
        return Ok(ReconcilerAction { requeue_after: None });
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
        Err(e) => {
            warn!("{pod_name}: could not find app label (will not retry): {e}");
            return Ok(ReconcilerAction { requeue_after: None });
        }
    };

    for sidecar in running_sidecars {
        let sidecar_name = sidecar.name;
        debug!("{pod_name}: found sidecar {sidecar_name}");
        let action = match ctx.get_ref().actions.get(&sidecar_name) {
            Some(action) => action,
            None => {
                warn!("{pod_name}: missing defined action: {sidecar_name}");
                UNSUPPORTED_SIDECARS
                    .with_label_values(&[&sidecar_name, &job_name, &namespace])
                    .inc();
                continue;
            }
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

    Ok(ReconcilerAction { requeue_after: None })
}

pub fn error_policy(_error: &Error, _ctx: Context<Data>) -> ReconcilerAction {
    ReconcilerAction {
        requeue_after: Some(Duration::from_secs(30)),
    }
}

// todo: i can't seem to mock the destroyer trait so that it gets used within reconcile. might need a refactor to enable better dependency injection.
/* #[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use crate::{api::MockDestroyer, reconciler::{Data, reconcile}};
    use k8s_openapi::{api::core::v1::{Pod, PodStatus, ContainerStatus, ContainerStateRunning, ContainerState, ContainerStateTerminated}, apimachinery::pkg::apis::meta::v1::Time, chrono::Utc};
    use kube::{api::ObjectMeta, Client, runtime::{events::Reporter, controller::Context}};
    #[tokio::test]
    async fn reconcile_test() {
        let mut destroyer = MockDestroyer::new();
        destroyer.expect_shutdown()
            .times(1)
            .returning(|_,_,_| Ok(()));

        let labels: BTreeMap<String, String> = BTreeMap::from([
            ("app".into(), "oh-no".into())
        ]);
        let pod = Pod {
            metadata: ObjectMeta {
                name: Some("oh-no".into()),
                labels: Some(labels),
                ..Default::default()
            },
            status: Some(PodStatus {
                container_statuses: Some(vec![
                    ContainerStatus {
                        name: "oh-no".into(),
                        state: Some(ContainerState {
                            terminated: Some(ContainerStateTerminated {
                                started_at: Some(Time(Utc::now())),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    ContainerStatus {
                        name: "cloudsql-proxy".into(),
                        state: Some(ContainerState {
                            running: Some(ContainerStateRunning {
                                started_at: Some(Time(Utc::now()))
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }
                ]),
                ..Default::default()
            }),
            ..Default::default()
        };

        let data = Data {
            actions: crate::actions::generate(),
            client: Client::try_default().await.unwrap(),
            reporter: Reporter {
                controller: "hahaha".into(),
                instance: Some("hahaha".into()),
            }
        };

        let arcpod = Arc::new(pod);

        let ret = reconcile(arcpod, Context::new(data)).await.unwrap();

        assert!(ret.requeue_after.is_none());
    }
} */
