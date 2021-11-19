use crate::actions::{Action, ActionType};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams};
use async_trait::async_trait;
use tracing::{info, error};

#[async_trait]
pub trait Destroying {
    async fn shutdown(&self, action: &Action, pod_name: &str, container_name: &str);
}

#[async_trait]
impl Destroying for Api<Pod> {
    async fn shutdown(&self, action: &Action, pod_name: &str, container_name: &str) {
        match action.action_type {
            ActionType::Exec => {
                let command: Vec<&str> = action.command.as_ref()
                    .unwrap() // we know that command is set at this point
                    .split(' ')
                    .collect();
                match self.exec(
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
            ActionType::Portforward => info!("I would have {} {} at port {} into {}@{} if I could!", 
                action.method.as_ref().unwrap(),
                action.path.as_ref().unwrap(),
                action.port.as_ref().unwrap(),
                container_name, 
                pod_name
            ),
            _ => ()
        };
    }
}