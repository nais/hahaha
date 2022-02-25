#[cfg(test)]
use mockall::automock;

use crate::actions::{Action, ActionType};
use anyhow::anyhow;
use async_trait::async_trait;
use hyper::{body, Body, Request};
use k8s_openapi::api::core::v1::Pod;
use kube::api::{Api, AttachParams};
use std::time::Duration;
use tracing::{debug, error, info};

/// Shutdown method for Apis with type Pod
#[cfg_attr(test, automock)]
#[async_trait]
pub trait Destroyer {
    /// Shuts down a container in a given pod with a given Action
    ///
    /// This is the primary public facing business function for this application
    async fn shutdown(&self, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()>;
}

/// Private trait for the actual business of shutting down pods
#[async_trait]
trait DestroyerActions {
    /// Helper to shut down a container via `exec`
    async fn shutdown_exec(&self, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()>;
    /// Helper to shut down a container via `portforward`
    async fn shutdown_portforward(&self, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()>;
}

#[async_trait]
impl Destroyer for Api<Pod> {
    async fn shutdown(&self, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()> {
        match action.action_type {
            ActionType::Exec => self.shutdown_exec(action, pod_name, container_name).await?,
            ActionType::Portforward => self.shutdown_portforward(action, pod_name, container_name).await?,
            _ => (),
        };
        Ok(())
    }
}

#[async_trait]
impl DestroyerActions for Api<Pod> {
    async fn shutdown_exec(&self, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()> {
        let command: Vec<&str> = action.command.as_ref().unwrap().split(' ').collect();
        debug!("{pod_name}: running command: {command:?}");
        match self
            .exec(
                pod_name,
                command.clone(),
                &AttachParams::default().container(container_name).stdout(false),
            )
            .await
        {
            Ok(_) => info!("{pod_name}: sent `{command:?}` to {container_name}",),
            Err(err) => return Err(anyhow!(format!("{pod_name}: exec failed in {container_name}: {err}"))),
        };
        Ok(())
    }

    async fn shutdown_portforward(&self, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()> {
        let port = action.port.unwrap();
        let mut pf = self.portforward(pod_name, &[port]).await?;
        let pf_ports = pf.ports();
        let stream = pf_ports[0].stream().unwrap();
        let (mut sender, connection) = hyper::client::conn::handshake(stream).await?;

        let inner_pod_name = pod_name.to_string();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                error!("{inner_pod_name}: error in portforward connection: {e}");
            }
        });

        let method = action.method.as_ref().unwrap().as_str();
        let path = action.path.as_ref().unwrap().as_str();
        let req = Request::builder()
            .uri(path)
            .header("Connection", "close")
            .header("Host", "127.0.0.1")
            .method(method)
            .body(Body::from(""))
            .unwrap();

        debug!("{pod_name}: sending HTTP request ({method} {path} at {port})");

        let req_future = sender.send_request(req);

        let (parts, body) = match tokio::time::timeout(Duration::from_secs(1), req_future).await {
            Ok(req) => req?.into_parts(),
            Err(_) => {
                return Err(anyhow!(format!(
                    "{pod_name}: HTTP request ({method} {path} at port {port}) failed: request timeout"
                )))
            }
        };
        let status_code = parts.status;
        debug!("{pod_name}: got status code {status_code}");
        if status_code != 200 {
            let body_bytes = body::to_bytes(body).await?;
            let body_str = std::str::from_utf8(&body_bytes)?;
            return Err(anyhow!(format!(
                "{pod_name}: HTTP request ({method} {path} at port {port}) failed: code {status_code}: {body_str}"
            )));
        } else {
            info!("{pod_name}: sent HTTP request `{method} {path}` at port {port} to {container_name}",)
        }
        Ok(())
    }
}
