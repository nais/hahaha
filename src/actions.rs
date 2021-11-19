use std::collections::BTreeMap;

pub enum ActionType {
    Portforward,
    Exec,
    None
}

impl Default for ActionType {
    fn default() -> Self { ActionType::None }
}

// This enum might be a little stupid. There should be better enums floating out there
// in HTTP client land, which I can use in my Action struct.
pub enum HTTPMethod {
    Get,
    Post
}

#[derive(Default)]
pub struct Action {
    pub action_type: ActionType,
    pub method: Option<HTTPMethod>,
    pub path: Option<String>,
    pub port: Option<usize>,
    pub command: Option<String>
}

// A comfy hardcoded map of all the sidecars that we know how to shut down
pub fn generate() -> BTreeMap<String, Action> {
    let mut map = BTreeMap::new();

    // containers that can be shut down via 
    map.insert("cloudsql-proxy".into(), Action {
        action_type: ActionType::Exec,
        command: Some("kill -s INT 1".into()),
        ..Default::default()
    });
    map.insert("vks-sidecar".into(), Action {
        action_type: ActionType::Exec,
        command: Some("/bin/kill -s INT 1".into()),
        ..Default::default()
    });
    map.insert("secure-logs-configmap-reload".into(), Action {
        action_type: ActionType::Exec,
        command: Some("/bin/killall configmap-reload".into()),
        ..Default::default()
    });

    // containers that require a portforward
    map.insert("linkerd-proxy".into(), Action {
        action_type: ActionType::Portforward,
        method: Some(HTTPMethod::Post),
        path: Some("/shutdown".into()),
        port: Some(4191),
        ..Default::default()
    });
    map.insert("secure-logs-fluentd".into(), Action {
        action_type: ActionType::Portforward,
        method: Some(HTTPMethod::Get),
        path: Some("/api/processes.killWorkers".into()),
        port: Some(24444),
        ..Default::default()
    });

    map
}
