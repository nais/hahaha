use anyhow::{anyhow, Result};
use k8s_openapi::api::core::v1::{ContainerStatus, Pod};

/// Public extension trait for `Pod`
pub trait Sidecars {
    /// Get all `ContainerStatus`es except the main application container for a `Pod`
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>>;
    /// Get the value of the `app` label in a Pod
    fn job_name(&self) -> anyhow::Result<String>;
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

/// Extension trait for `ContainerStatus`
trait ContainerState {
    /// Helper to determine if a Container is terminated
    fn is_terminated(&self) -> bool;
}

impl Sidecars for Pod {
    fn sidecars(&self) -> anyhow::Result<Vec<ContainerStatus>> {
        let sidecars = self.running_sidecars()?;
        if sidecars.is_empty() {
            // if there's nothing to be found, we're probably still starting up.
            return Ok(sidecars);
        }
        let main_container = self.main_container()?;
        if !main_container.is_terminated() {
            return Ok(Vec::new());
        }
        Ok(sidecars)
    }

    fn job_name(&self) -> anyhow::Result<String> {
        let Some(labels) = &self.metadata.labels else {
            return Err(anyhow!("no labels found on pod"));
        };
        let Some(app_name) = labels.get("app") else {
            return Err(anyhow!("no app name found on pod"));
        };
        Ok(app_name.into())
    }
}

impl SidecarStates for Pod {
    fn running_sidecars(&self) -> Result<Vec<ContainerStatus>> {
        let app = JobPod::from(self)?;
        Ok(app
            .statuses
            .iter()
            .filter(|c| c.name != app.name && !c.is_terminated())
            .cloned()
            .collect())
    }

    fn main_container(&self) -> Result<ContainerStatus> {
        let app = JobPod::from(self)?;
        app.statuses
            .iter()
            .find(|c| c.name == app.name)
            .map_or_else(|| Err(anyhow!("couldn't determine main containter")), |c| Ok(c.clone()))
    }
}

struct JobPod {
    pub name: String,
    pub statuses: Vec<ContainerStatus>,
}

impl JobPod {
    pub fn from(pod: &Pod) -> Result<Self> {
        let app_name = &pod
            .metadata
            .labels
            .as_ref()
            .ok_or_else(|| anyhow!("no labels found on pod"))?
            .get("app")
            .ok_or_else(|| anyhow!("no app name found on pod"))?;

        let container_statuses: Vec<ContainerStatus> = pod
            .status
            .as_ref()
            .ok_or_else(|| anyhow!("no spec found on pod"))?
            .container_statuses
            .as_ref()
            .map_or_else(Vec::new, Clone::clone);

        Ok(Self {
            name: (*app_name).to_string(),
            statuses: container_statuses,
        })
    }
}

impl ContainerState for ContainerStatus {
    // We could probably see if the termination reason is
    // a good one to avoid taking unnecessary measures.. but whatever
    fn is_terminated(&self) -> bool {
        self.state.as_ref().map_or(false, |c| c.terminated.is_some())
    }
}
