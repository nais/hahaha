use k8s_openapi::api::core::v1::{Pod, ContainerStatus};
use anyhow::{anyhow, Result};

pub struct AppPod {
    pub name: String,
    pub statuses: Vec<ContainerStatus>
}

impl AppPod {
    // A little module-private implementation to avoid a little duplication!
    pub fn from(pod: &Pod) -> Result<AppPod> {
        let labels = match &pod.metadata.labels {
            Some(l) => l,
            None => return Err(anyhow!("No labels found on pod"))
        };
        let app_name = match labels.get("app") {
            Some(name) => name,
            None => return Err(anyhow!("No app name found on pod"))
        };
        let pod_status = match &pod.status {
            Some(spec) => spec,
            None => return Err(anyhow!("No spec found on pod"))
        };

        let container_statuses = match &pod_status.container_statuses {
            Some(status) => status,
            None => return Err(anyhow!("No container statuses found on pod"))
        };

        Ok(AppPod {
            name: app_name.into(),
            statuses: container_statuses.to_vec()
        })
    }
}
