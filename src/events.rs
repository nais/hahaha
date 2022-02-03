use chrono::Utc;
use k8s_openapi::{
    api::{core::v1::ObjectReference, events::v1::Event},
    apimachinery::pkg::apis::meta::v1::{MicroTime, Time},
};
use kube::{api::PostParams, core::ObjectMeta, runtime::events::Reporter, Api, Client};

/// A slightly modified version of kube::runtime::events::Recorder
///
/// Published events also have deprecated_first_timestamp and deprecated_last_timestamp set
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
            deprecated_count: None,
            deprecated_first_timestamp: Some(Time(now.clone())),
            deprecated_last_timestamp: Some(Time(now.clone())),
            deprecated_source: None,
            event_time: MicroTime(now.clone()),
            regarding: Some(self.reference.clone()),
            note: Some(message),
            metadata: ObjectMeta {
                namespace: self.reference.namespace.clone(),
                generate_name: Some(format!("{}-", self.reporter.controller)),
                ..Default::default()
            },
            reporting_controller: Some(self.reporter.controller.clone()),
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


#[tokio::test]
async fn event_timestamps_are_set() -> anyhow::Result<()> {
    let client = Client::try_default().await?;
    let reporter = Reporter {
        controller: "hahaha".into(),
        instance: Some("hahaha-1234".into()),
    };
    let recorder = Recorder::new(
        client.clone(),
        reporter.clone(),
        ObjectReference {
            api_version: None,
            field_path: None,
            kind: None,
            name: None,
            namespace: None,
            resource_version: None,
            uid: None,
        },
    );

    let event = recorder.event("Normal".into(), "blah blah".into());
    let first_timestamp = event.deprecated_first_timestamp;
    let last_timestamp = event.deprecated_last_timestamp;
    let event_time = event.event_time;

    assert!(first_timestamp.is_some());
    assert!(last_timestamp.is_some());

    assert_eq!(first_timestamp, last_timestamp);

    let f_ts = &serde_json::to_string(&first_timestamp.unwrap())?[1..20];
    let l_ts = &serde_json::to_string(&last_timestamp.unwrap())?[1..20];
    let e_t = &serde_json::to_string(&event_time)?[1..20];

    assert!(chrono::NaiveDateTime::parse_from_str(e_t, "%Y-%m-%dT%H:%M:%S").is_ok());
    assert_eq!(f_ts, l_ts);
    assert_eq!(e_t, l_ts);

    Ok(())
}
