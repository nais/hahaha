use chrono::Utc;
use k8s_openapi::{
    api::{core::v1::Event, core::v1::ObjectReference},
    apimachinery::pkg::apis::meta::v1::Time,
};
use kube::{api::PostParams, core::ObjectMeta, runtime::events::Reporter, Api, Client};

/// A slightly modified version of kube::runtime::events::Recorder
///
/// The Recorder uses k8s_openapi::api::core::v1::Event instead of k8s_openapi::api::events::v1::Event.
/// This is to better align with ~v1.19 API versions, whereas later Kubernetes versions prefer the other struct.
/// TODO Once we upgrade to a cluster version which prefers events::v1::Event, we use kube-rs' Recorder instead
pub struct Recorder {
    events: Api<Event>,
    reporter: Reporter,
    reference: ObjectReference,
}

impl Recorder {
    /// Create a new recorder that can publish events for one specific object
    #[must_use]
    pub fn new(client: Client, reporter: Reporter, reference: ObjectReference) -> Self {
        let events = match reference.namespace.as_ref() {
            None => Api::all(client),
            Some(namespace) => Api::namespaced(client, namespace),
        };
        Self {
            events,
            reporter,
            reference,
        }
    }

    /// Publish a Killing event with the type Normal
    pub async fn info(&self, message: String) -> Result<(), kube::Error> {
        self.events
            .create(&PostParams::default(), &self.event("Normal".into(), message))
            .await?;
        Ok(())
    }

    /// Publish Killing event with the type Warning
    pub async fn warn(&self, message: String) -> Result<(), kube::Error> {
        self.events
            .create(&PostParams::default(), &self.event("Warning".into(), message))
            .await?;
        Ok(())
    }

    /// Helper method to create an event
    fn event(&self, type_: String, message: String) -> Event {
        let now = Utc::now();
        Event {
            action: Some("Killing".into()),
            reason: Some("Killing".into()),
            count: None,
            first_timestamp: Some(Time(now.clone())),
            last_timestamp: Some(Time(now.clone())),
            source: None,
            event_time: None,
            involved_object: self.reference.clone(),
            message: Some(message),
            metadata: ObjectMeta {
                namespace: self.reference.namespace.clone(),
                generate_name: Some(format!("{}-", self.reporter.controller)),
                ..Default::default()
            },
            reporting_component: Some(self.reporter.controller.clone()),
            reporting_instance: Some(
                self.reporter
                    .instance
                    .clone()
                    .unwrap_or_else(|| self.reporter.controller.clone()),
            ),
            series: None,
            type_: Some(type_),
            related: None,
        }
    }
}
