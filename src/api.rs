#[cfg(test)]
use mockall::automock;

use crate::actions::Action;
use anyhow::anyhow;
use async_trait::async_trait;
use hyper::http::Method;
use hyper::{body, Body, Request, Uri};
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

#[async_trait]
impl Destroyer for Api<Pod> {
    async fn shutdown(&self, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()> {
        shutdown_pod(self, action, pod_name, container_name).await
    }
}

async fn shutdown_pod(pod: &Api<Pod>, action: &Action, pod_name: &str, container_name: &str) -> anyhow::Result<()> {
    match action {
        Action::Exec(command) => shutdown_exec(pod, command, pod_name, container_name).await,
        Action::Portforward(method, path, port) => {
            shutdown_portforward(pod, method, path, *port, pod_name, container_name).await
        }
    }
}

async fn shutdown_exec(
    pod: &Api<Pod>,
    command: &Vec<String>,
    pod_name: &str,
    container_name: &str,
) -> anyhow::Result<()> {
    debug!("{pod_name}: running command: {command:?}");
    match pod
        .exec(
            pod_name,
            command,
            &AttachParams::default().container(container_name).stdout(false),
        )
        .await
    {
        Ok(_) => info!("{pod_name}: sent `{command:?}` to {container_name}",),
        Err(err) => return Err(anyhow!(format!("{pod_name}: exec failed in {container_name}: {err}"))),
    };
    Ok(())
}

async fn shutdown_portforward(
    pod: &Api<Pod>,
    method: &Method,
    path: &Uri,
    port: u16,
    pod_name: &str,
    container_name: &str,
) -> anyhow::Result<()> {
    let mut pf = pod.portforward(pod_name, &[port]).await?;
    let (mut sender, connection) = match pf.take_stream(port) {
        None => return Err(anyhow!(format!("Unable to attach to port: {port}"))),
        Some(s) => hyper::client::conn::handshake(s).await?,
    };

    let inner_pod_name = pod_name.to_string();
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            error!("{inner_pod_name}: error in portforward connection: {e}");
        }
    });

    let req = Request::builder()
        .uri(path)
        .header("Connection", "close")
        .header("Host", "127.0.0.1")
        .method(method)
        .body(Body::from(""))?;

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
    }
    info!("{pod_name}: sent HTTP request `{method} {path}` at port {port} to {container_name}",);
    Ok(())
}
