use anyhow::{anyhow, Result};
use k8s_openapi::api::core::v1::{ContainerStatus, Pod};

/// Public extension trait for `Pod`
pub trait Sidecars {
    /// Get all `ContainerStatus`es except the main application container for a `Pod`
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>>;
    fn app_name(&self) -> anyhow::Result<String>;
}

/// Extension trait for `Pod`
///
/// Only used in the Sidecars trait
trait SidecarStates {
    /// Get all `ContainerStatus`es which are not terminated in a Pod
    fn running_sidecars(&self) -> Result<Vec<ContainerStatus>>;
    /// Get the `ContainerStatus` that matches the `app` label in a Pod
    fn main_container(&self) -> Result<ContainerStatus>;
}

/// Extension trait for ContainerStatus
trait ContainerState {
    /// Helper to determine if a Container is terminated
    fn is_terminated(&self) -> bool;
}

impl Sidecars for Pod {
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>> {
        let sidecars = self.running_sidecars()?;
        if sidecars.len() == 0 {
            // if there's nothing to be found, we're probably still starting up.
            return Ok(sidecars);
        }
        let main_container = self.main_container()?;
        if !main_container.is_terminated() {
            return Ok(Vec::new());
        }
        Ok(sidecars)
    }

    fn app_name(&self) -> anyhow::Result<String> {
        Ok(App::from(self)?.name)
    }
}

impl SidecarStates for Pod {
    fn running_sidecars(&self) -> Result<Vec<ContainerStatus>> {
        let app = App::from(self)?;
        Ok(app
            .statuses
            .iter()
            .filter(|c| c.name != app.name && !c.is_terminated())
            .cloned()
            .collect())
    }

    fn main_container(&self) -> Result<ContainerStatus> {
        let app = App::from(self)?;
        match app.statuses.iter().find(|c| c.name == app.name) {
            Some(c) => Ok(c.clone()),
            None => Err(anyhow!("Couldn't determine main container")),
        }
    }
}

struct App {
    pub name: String,
    pub statuses: Vec<ContainerStatus>,
}

impl App {
    pub fn from(pod: &Pod) -> Result<App> {
        let labels = match &pod.metadata.labels {
            Some(l) => l,
            None => return Err(anyhow!("No labels found on pod")),
        };
        let app_name = match labels.get("app") {
            Some(name) => name,
            None => return Err(anyhow!("No app name found on pod")),
        };
        let pod_status = match &pod.status {
            Some(spec) => spec,
            None => return Err(anyhow!("No spec found on pod")),
        };

        let container_statuses: Vec<ContainerStatus> = match &pod_status.container_statuses {
            Some(status) => status.to_vec(),
            None => Vec::new(), // if no container statuses are found, return an empty Vec. we're probably starting up
        };

        Ok(App {
            name: app_name.into(),
            statuses: container_statuses,
        })
    }
}

impl ContainerState for ContainerStatus {
    // We could probably see if the termination reason is
    // a good one to avoid taking unnecessary measures.. but whatever
    fn is_terminated(&self) -> bool {
        let last_state = match &self.state {
            Some(state) => state,
            None => return false,
        };

        last_state.terminated.is_some()
    }
}
