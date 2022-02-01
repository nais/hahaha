use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams},
    runtime::{
        events::{Event, EventType, Recorder, Reporter},
        utils::try_flatten_applied,
        watcher,
    },
    Client, Resource, ResourceExt,
};
use tracing::{info, warn};

mod actions;
mod api;
mod pod;

use api::Destroyer;
use pod::Sidecars;

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
        instance: Some("hahaha-1234".into()), // todo get instance from cluster when deployed
    };

    let mut ew = try_flatten_applied(watcher(pods, lp)).boxed();

    while let Some(pod) = ew.try_next().await? {
        let running_sidecars = pod.sidecars().unwrap_or_else(|err| {
            info!("Getting running sidecars for {}: {}", pod.name(), err);
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

        info!(
            "{} in namespace {} needs help shutting down some residual containers!",
            pod.name(),
            namespace
        );

        for sidecar in running_sidecars {
            let sidecar_name = sidecar.name;
            let action = match actions.get(&sidecar_name) {
                Some(action) => action,
                None => {
                    warn!(
                        "I don't know how to shut down {} (in {} in namespace {})",
                        sidecar_name,
                        pod.name(),
                        namespace
                    );
                    continue;
                }
            };
            let res = api.shutdown(action, &pod.name(), &sidecar_name).await;
            if res.is_ok() {
                recorder
                    .publish(Event {
                        type_: EventType::Normal,
                        action: "Killing".into(),
                        reason: "Killing".into(),
                        note: Some(format!("Successfully shut down container {}", sidecar_name)),
                        secondary: None,
                    })
                    .await?;
            } else {
                recorder
                    .publish(Event {
                        type_: EventType::Warning,
                        action: "Killing".into(),
                        reason: "Killing".into(),
                        note: Some(format!("Unsuccessfully shut down container {}", sidecar_name)),
                        secondary: None,
                    })
                    .await?;
            }
        }
    }
    Ok(())
}
