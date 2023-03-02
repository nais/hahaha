use std::{collections::BTreeMap, sync::Arc, time::Duration};

use k8s_openapi::api::core::v1::Pod;
use kube::{
    runtime::{
        controller::{Action as ReconcilerAction},
        events::{Reporter, Recorder, Event, EventType},
    },
    Api, Client, Resource, ResourceExt,
};
use thiserror::Error;
use tracing::{debug, warn};

use crate::{actions::Action, api::Destroyer, pod::Sidecars, prometheus::*};

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

pub async fn reconcile(pod: Arc<Pod>, ctx: Arc<Data>) -> Result<ReconcilerAction, Error> {
    let namespace = match pod.namespace() {
        Some(namespace) => namespace,
        None => "default".into(),
    };
    let api: Api<Pod> = Api::namespaced(ctx.client.clone(), &namespace);
    reconcile_inner(api, pod, ctx).await
}

pub async fn reconcile_inner(
    api: impl Destroyer,
    pod: Arc<Pod>,
    ctx: Arc<Data>,
) -> Result<ReconcilerAction, Error> {
    let pod_name = pod.name_any();
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
        return Ok(ReconcilerAction::await_change());
    }

    // set up a recorder for publishing events to the Pod
    let recorder = Recorder::new(
        ctx.client.clone(),
        ctx.reporter.clone(),
        pod.object_ref(&()),
    );

    debug!("{pod_name}: needs help shutting down some residual containers");

    let job_name = match pod.job_name() {
        Ok(name) => name,
        Err(e) => {
            // this will never occur: running_sidecars will return on the same case
            warn!("{pod_name}: could not find app label (will not retry): {e}");
            return Ok(ReconcilerAction::await_change());
        }
    };

    for sidecar in running_sidecars {
        let sidecar_name = sidecar.name;
        debug!("{pod_name}: found sidecar {sidecar_name}");
        let action = match ctx.actions.get(&sidecar_name) {
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
                .publish(Event {
                    action: "Killing".into(),
                    reason: "Killing".into(),
                    note: Some(format!("Unsuccessfully shut down container {sidecar_name}: {err}").into()),
                    type_: EventType::Warning,
                    secondary: None
                }).await
            {
                warn!("{pod_name}: couldn't publish Kubernetes Event: {e}");
                TOTAL_UNSUCCESSFUL_EVENT_POSTS.inc();
            }
            FAILED_SIDECAR_SHUTDOWNS
                .with_label_values(&[&sidecar_name, &job_name, &namespace])
                .inc();
            return Err(Error::SidecarShutdownFailed(pod_name, sidecar_name, err));
        }
        if let Err(e) = recorder.publish(Event {
            action: "Killing".into(),
            reason: "Killing".into(),
            note: Some(format!("Shut down container {sidecar_name}").into()),
            type_: EventType::Normal,
            secondary: None
        }).await {
            warn!("{pod_name}: couldn't publish Kubernetes Event: {e}");
            TOTAL_UNSUCCESSFUL_EVENT_POSTS.inc();
        }
        SIDECAR_SHUTDOWNS
            .with_label_values(&[&sidecar_name, &job_name, &namespace])
            .inc();
    }

    Ok(ReconcilerAction::await_change())
}

pub fn error_policy(_pod: Arc<Pod>, _error: &Error, _ctx: Arc<Data>) -> ReconcilerAction {
    ReconcilerAction::requeue(Duration::from_secs(30))
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, sync::Arc};

    use crate::{
        api::MockDestroyer,
        reconciler::{reconcile_inner, Data},
    };
    use hyper::Uri;
    use k8s_openapi::{
        api::core::v1::{
            ContainerState, ContainerStateRunning, ContainerStateTerminated, ContainerStatus, Pod, PodStatus,
        },
        apimachinery::pkg::apis::meta::v1::Time,
        chrono::Utc,
    };
    use kube::{
        api::ObjectMeta,
        client::ConfigExt,
        runtime::events::Reporter,
        Client, Config,
    };
    use tower::ServiceBuilder;

    /// creates a bogus kube client that doesn't connect anywhere useful
    fn make_data() -> Data {  
        let config = Config::new("/".parse::<Uri>().unwrap());
        let service = ServiceBuilder::new()
            .layer(config.base_uri_layer())
            .option_layer(config.auth_layer().unwrap())
            .service(hyper::Client::new());

        Data {
            actions: crate::actions::generate(),
            client: Client::new(service, config.default_namespace),
            reporter: Reporter {
                controller: "hahaha".into(),
                instance: Some("hahaha".into()),
            },
        }
    }

    fn make_pod(
        name: String,
        labels: Option<BTreeMap<String, String>>,
        mut extra_containers: Vec<ContainerStatus>,
    ) -> Pod {
        let mut container_statuses = vec![ContainerStatus {
            name: name.clone(),
            state: Some(ContainerState {
                terminated: Some(ContainerStateTerminated {
                    started_at: Some(Time(Utc::now())),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }];
        container_statuses.append(&mut extra_containers);

        Pod {
            metadata: ObjectMeta {
                name: Some(name),
                labels,
                ..Default::default()
            },
            status: Some(PodStatus {
                container_statuses: Some(container_statuses),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn reconcile_ok_on_successful_shutdown() {
        let mut destroyer = MockDestroyer::new();
        destroyer.expect_shutdown().times(1).returning(|_, _, _| Ok(()));

        let name: String = String::from("oh-no");

        let labels: BTreeMap<String, String> = BTreeMap::from([("app".into(), name.clone())]);
        let extra_containers = vec![ContainerStatus {
            name: "cloudsql-proxy".into(),
            state: Some(ContainerState {
                running: Some(ContainerStateRunning {
                    started_at: Some(Time(Utc::now())),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }];

        let ret = reconcile_inner(
            destroyer,
            Arc::new(make_pod(name, Some(labels), extra_containers)),
            Arc::new(make_data()),
        )
        .await;

        assert!(ret.is_ok());
    }

    #[tokio::test]
    async fn reconcile_ok_on_no_running_sidecars() {
        let mut destroyer = MockDestroyer::new();
        destroyer.expect_shutdown().times(0).returning(|_, _, _| Ok(()));

        let name: String = String::from("oh-no");

        let labels: BTreeMap<String, String> = BTreeMap::from([("app".into(), name.clone())]);

        let ret = reconcile_inner(
            destroyer,
            Arc::new(make_pod(name, Some(labels), vec![])),
            Arc::new(make_data()),
        )
        .await;

        assert!(ret.is_ok());
    }

    #[tokio::test]
    async fn reconcile_err_on_failed_shutdown() {
        let mut destroyer = MockDestroyer::new();
        destroyer
            .expect_shutdown()
            .times(1)
            .returning(|_, _, _| Err(anyhow::anyhow!("couldn't shutdown!")));
        let name = String::from("oh-no");

        let labels: BTreeMap<String, String> = BTreeMap::from([("app".into(), name.clone())]);
        let extra_containers = vec![ContainerStatus {
            name: "cloudsql-proxy".into(),
            state: Some(ContainerState {
                running: Some(ContainerStateRunning {
                    started_at: Some(Time(Utc::now())),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }];

        let ret = reconcile_inner(
            destroyer,
            Arc::new(make_pod(name.clone(), Some(labels), extra_containers)),
            Arc::new(make_data()),
        )
        .await
        .unwrap_err();
        assert_eq!(
            ret.to_string(),
            format!("{name}: could not shut down sidecar cloudsql-proxy: couldn't shutdown!")
        );
    }

    #[tokio::test]
    async fn reconcile_err_on_misconfigured_pod() {
        let mut destroyer = MockDestroyer::new();
        destroyer.expect_shutdown().times(0).returning(|_, _, _| Ok(()));

        let name: String = String::from("oh-no");

        let labels: BTreeMap<String, String> = BTreeMap::new();

        let ret = reconcile_inner(
            destroyer,
            Arc::new(make_pod(name.clone(), Some(labels), vec![])),
            Arc::new(make_data()),
        )
        .await
        .unwrap_err();

        assert_eq!(
            ret.to_string(),
            format!("{name}: could not get running sidecars: no app name found on pod")
        );
    }
}
