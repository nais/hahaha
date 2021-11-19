use super::app_pod::AppPod;
use k8s_openapi::api::core::v1::{Pod, ContainerStatus};
use anyhow::{anyhow, Result};

pub trait Sidecars {
    fn running_sidecars(&self) -> Result<Vec<ContainerStatus>>;
    fn main_container(&self) -> Result<ContainerStatus>;
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>>;
}

impl Sidecars for Pod {
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>> {
        let main_container = self.main_container()?;
        if !main_container.is_terminated() {
            return Ok(Vec::new())
        }
        self.running_sidecars()
    }

    fn running_sidecars(&self) -> Result<Vec<ContainerStatus>> {
        let app = AppPod::from(self)?;
        Ok(app.statuses.iter()
            .filter(|c| c.name != app.name && !c.is_terminated())
            .cloned()
            .collect())
    }

    fn main_container(&self) -> Result<ContainerStatus> {
        let app = AppPod::from(self)?;
        match app.statuses.iter().find(|c| c.name == app.name) {
            Some(c) => Ok(c.clone()),
            None => Err(anyhow!("Couldn't determine main container"))
        }
    }
}

pub trait ContainerStateExt {
    fn is_terminated(&self) -> bool;
}

impl ContainerStateExt for ContainerStatus {
    // Determines if a container is terminated.
    // We could probably see if the termination reason is
    // a good one to avoid taking unnecessary measures.. but whatever
    fn is_terminated(&self) -> bool {
        let last_state = match &self.state {
            Some(state) => state,
            None => return false
        };

        last_state.terminated.is_some()
    }
}