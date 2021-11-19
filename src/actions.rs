use std::collections::BTreeMap;
use anyhow::{anyhow, Result};

pub enum ActionType {
    Portforward,
    Exec,
    None
}

impl Default for ActionType {
    fn default() -> Self { ActionType::None }
}

#[derive(Default)]
pub struct Action {
    pub action_type: ActionType,
    pub method: Option<String>,
    pub path: Option<String>,
    pub port: Option<usize>,
    pub command: Option<String>
}

impl Action {
    fn validate(&self) -> Result<()> {
        match self.action_type {
            ActionType::Exec => {
                if self.command.is_none() {
                    return Err(anyhow!("Command is required to be set with ActionType::Exec"))
                }
            },
            ActionType::Portforward => {
                if self.method.is_none() {
                    return Err(anyhow!("Method is required to be set with ActionType::Portforward"))
                }
                if self.path.is_none() {
                    return Err(anyhow!("Path is required to be set with ActionType::Portforward"))
                }
                if self.port.is_none() {
                    return Err(anyhow!("Port is required to be set with ActionType::Portforward"))
                }
            },
            _ => ()
        };
        Ok(())
    }
}

// A comfy hardcoded map of all the sidecars that we know how to shut down.
// TODO: probably put a similar structure into some yaml
pub fn generate() -> Result<BTreeMap<String, Action>> {
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
        method: Some("POST".into()),
        path: Some("/shutdown".into()),
        port: Some(4191),
        ..Default::default()
    });
    map.insert("secure-logs-fluentd".into(), Action {
        action_type: ActionType::Portforward,
        method: Some("GET".into()),
        path: Some("/api/processes.killWorkers".into()),
        port: Some(24444),
        ..Default::default()
    });

    // one last sanity check before we send it off
    for action in map.values() {
        action.validate()?;
    }

    Ok(map)
}
