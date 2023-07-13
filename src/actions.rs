use hyper::http::Method;
use hyper::Uri;
use std::{collections::BTreeMap};
/// Generate the action `BTreeMap`
///
/// Modify this function to add or remove sidecar definitions and their associated shutdown procedures.
pub fn generate() -> BTreeMap<String, Action> {
    BTreeMap::from([
        (
            "cloudsql-proxy".into(),
            Action::Exec("kill -s INT 1".split(' ').map(String::from).collect()),
        ),
        (
            "vks-sidecar".into(),
            Action::Exec("/bin/kill -s INT 1".split(' ').map(String::from).collect()),
        ),
        (
            "secure-logs-configmap-reload".into(),
            Action::Exec("/bin/killall configmap-reload".split(' ').map(String::from).collect()),
        ),
        (
            "linkerd-proxy".into(),
            Action::Portforward(Method::POST, "/shutdown".parse::<Uri>().unwrap(), 4191),
        ),
        (
            "secure-logs-fluentd".into(),
            Action::Portforward(Method::GET, "/api/processes.killWorkers".parse::<Uri>().unwrap(), 24444),
        ),
    ])
}

#[derive(Debug)]
pub enum Action {
    Portforward(Method, Uri, u16),
    Exec(Vec<String>),
}
