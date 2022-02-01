use std::collections::BTreeMap;

/// Generate the action `BTreeMap`
///
/// Modify this function to add or remove sidecar definitions and their associated shutdown procedures.
pub fn generate() -> BTreeMap<String, Action> {
    let mut actions = BTreeMap::new();

    actions.exec("cloudsql-proxy", "kill -s INT 1");
    actions.exec("vks-sidecar", "/bin/kill -s INT 1");
    actions.exec("secure-logs-configmap-reload", "/bin/killall configmap-reload");
    actions.portforward("linkerd-proxy", "POST", "/shutdown", 4191);
    actions.portforward("secure-logs-fluentd", "GET", "/api/processes.killWorkers", 24444);
    actions
}

#[derive(Debug)]
pub enum ActionType {
    Portforward,
    Exec,
    None,
}

impl Default for ActionType {
    fn default() -> Self {
        ActionType::None
    }
}

#[derive(Default, Debug)]
pub struct Action {
    pub action_type: ActionType,
    pub method: Option<String>,
    pub path: Option<String>,
    pub port: Option<u16>,
    pub command: Option<String>,
}

/// Helper trait for inserting different `Action`s with different `ActionType`s into a `BTreeMap`
/// 
/// Using this trait will allow us to catch simple misconfigurations in Actions during compile time.
trait ActionInsertions {
    /// Inserts an action with `ActionType::Exec` into a `BTreeMap`
    fn exec(&mut self, target_container: &str, command: &str);
    /// Inserts an action with `ActionType::Portforward` into a `BTreeMap`
    fn portforward(&mut self, target_container: &str, method: &str, path: &str, port: u16);
}

impl ActionInsertions for BTreeMap<String, Action> {
    fn exec(&mut self, target_container: &str, command: &str) {
        self.insert(
            target_container.into(),
            Action {
                action_type: ActionType::Exec,
                command: Some(command.into()),
                ..Default::default()
            },
        );
    }

    fn portforward(&mut self, target_container: &str, method: &str, path: &str, port: u16) {
        self.insert(
            target_container.into(),
            Action {
                action_type: ActionType::Portforward,
                method: Some(method.into()),
                path: Some(path.into()),
                port: Some(port.into()),
                ..Default::default()
            },
        );
    }
}

#[test]
fn generate_len_5() {
    let actions = generate();
    assert_eq!(5, actions.len())
}
