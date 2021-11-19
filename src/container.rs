use k8s_openapi::api::core::v1::ContainerStatus;

pub trait HahahaContainer {
    fn is_terminated(&self) -> bool;
}


impl HahahaContainer for ContainerStatus {
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