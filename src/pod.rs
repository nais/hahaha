use k8s_openapi::api::core::v1::{Pod, ContainerStatus};
use anyhow::{anyhow, Result};
use crate::{HahahaContainer};

pub trait HahahaPod {
    fn running_sidecars(&self) -> Result<Vec<ContainerStatus>>;
    fn main_container(&self) -> Result<ContainerStatus>;
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>>;
}

impl HahahaPod for Pod {
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>> {
        let main_container = self.main_container()?;
        if !main_container.is_terminated() {
            return Ok(Vec::new())
        }
        self.running_sidecars()
    }

    fn running_sidecars(&self) -> Result<Vec<ContainerStatus>> {
        let haha = HahaPod::from(&self)?;
        Ok(haha.statuses.iter()
            .filter(|c| c.name != haha.name && !c.is_terminated())
            .map(|c| c.clone())
            .collect())
    }

    fn main_container(&self) -> Result<ContainerStatus> {
        let haha = HahaPod::from(&self)?;
        match haha.statuses.iter().find(|c| c.name == haha.name) {
            Some(c) => Ok(c.clone()),
            None => Err(anyhow!("Couldn't determine main container"))
        }
    }

}

struct HahaPod {
    pub name: String,
    pub statuses: Vec<ContainerStatus>
}

impl HahaPod {
    // A little module-private implementation to avoid a little duplication!
    fn from(pod: &Pod) -> Result<HahaPod> {
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

        Ok(HahaPod {
            name: app_name.into(),
            statuses: container_statuses.to_vec()
        })
    }
}
