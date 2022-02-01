use super::containers::Sidecars;
use crate::{actions::Action, haha::api::Destroying};
use async_trait::async_trait;
use k8s_openapi::api::core::v1::{Pod};
use kube::{api::Api, Client, ResourceExt};
use std::collections::BTreeMap;
use tracing::{info, warn};

#[async_trait]
pub trait Handleable {
    async fn handle(self, client: &Client, actions: &BTreeMap<String, Action>) -> anyhow::Result<()>;
}

#[async_trait]
impl Handleable for Pod {
    async fn handle(self, client: &Client, actions: &BTreeMap<String, Action>) -> anyhow::Result<()> {
        let running_sidecars = self.sidecars().unwrap_or_else(|err| {
            // this might also be hit on a job's startup
            // todo: maybe fix it and eliminate unnecessary logs?
            info!("Getting running sidecars for {}: {}", self.name(), err);
            Vec::new()
        });
        if running_sidecars.is_empty() {
            // To avoid setting up a useless api
            return Ok(());
        }

        info!("{} needs help shutting down some residual containers!", self.name());
        // we have to create a namespaced api to the target pod's namespace 
        // in order to later `exec` (inside perform), since we can't pass a namespace into `exec`.
        // idk if this creation of a new api for every eligible pod is expensive..
        let namespace = match self.namespace() {
            Some(namespace) => namespace,
            None => "default".into()
        };
        let api: Api<Pod> = Api::namespaced(client.clone(), &namespace);

        for sidecar in running_sidecars {
            let name = sidecar.name;
            let action = match actions.get(&name) {
                Some(action) => action,
                None => {
                    warn!("I don't know how to shut down {} (in {})", name, self.name());
                    continue;
                }
            };
            api.shutdown(action, &self.name(), &name).await?;
        }
        Ok(())
    }
}