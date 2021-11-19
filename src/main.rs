use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams, AttachParams},
    runtime::{utils::try_flatten_applied, watcher},
    Client,
    ResourceExt
};
use tracing::{info, error, warn};
use tracing_subscriber;
use std::collections::BTreeMap;

mod pod;
mod container;
mod actions;

use pod::HahahaPod;
use container::HahahaContainer;
use actions::{Action, ActionType};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,kube=warn");
    tracing_subscriber::fmt::init();
    
    let actions = actions::generate();
    let client = Client::try_default().await?;

    let pods: Api<Pod> = Api::all(client.clone());
    let lp = ListParams::default()
        .timeout(30)
        // I'm leaning towards using labels as a filter for target pods.. so I'm using it here
        .labels("nais.io/ginuudan=enabled");

    let mut ew = try_flatten_applied(watcher(pods, lp)).boxed();

    while let Some(pod) = ew.try_next().await? {
        handle_pod(pod, &client, &actions).await;
    }
    Ok(())
}


// I haven't decided if I should move this into a different module
// I could use async-traits to extend Pod and make our api look a bit cuter, but.. not yet, maybe
async fn handle_pod(pod: Pod, client: &Client, actions: &BTreeMap<String, Action>) {
    let running_sidecars = pod.sidecars().unwrap_or_else(|err| {
        error!("Getting running sidecars for {}: {}", pod.name(), err);
        Vec::new()
    });
    if running_sidecars.len() == 0 {
        // To avoid setting up a useless api
        return;
    }

    // we have to create a namespaced api to the target pod's namespace 
    // in order to later `exec` (inside perform), since we can't pass a namespace into `exec`.
    // idk if this creation of a new api for every eligible pod is expensive..
    let namespace = match pod.namespace() {
        Some(namespace) => namespace,
        None => "default".into()
    };
    let api: Api<Pod> = Api::namespaced(client.clone(), &namespace);

    for sidecar in running_sidecars {
        let name = sidecar.name;
        let action = match actions.get(&name) {
            Some(action) => action,
            None => {
                warn!("I don't know how to shut down {} (in {})", name, pod.name());
                continue;
            }
        };
        perform(&api, action, &pod.name(), &name).await;
    }
}

// Likewise here, this could be an async-trait on &Api<Pod> which also should need to be
// namespaced.. meh
async fn perform(api: &Api<Pod>, action: &Action, pod_name: &str, container_name: &str) {
    match action.action_type {
        ActionType::Exec => {
            let command = action.command.as_ref().unwrap().split(" ").collect::<Vec<&str>>();
            match api.exec(
                pod_name,
                command,
                &AttachParams::default()
                    .container(container_name)
                    .stdout(false)
            ).await {
                Ok(_) => (),
                Err(err) => {
                    error!("Something bad happened while trying to exec into {} ({}): {}", 
                        pod_name, container_name, err);
                }
            };
        },
        ActionType::Portforward => info!("haha! portforward @ {} ({})!", pod_name, container_name),
        _ => ()
    };
}