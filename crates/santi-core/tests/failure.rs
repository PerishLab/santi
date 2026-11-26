const SCENARIO: &str = "fixture/model/execution";

use async_trait::async_trait;
use futures_util::stream;
use santi_core::Category;
use santi_core::service::{self, Service};
use santi_core::{message, strand, turn};
use santi_provider::{Event, Item, Metadata, Provider, Request, Streaming};
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, sleep};

#[path = "failure/finish.rs"]
mod finish;
#[path = "failure/more.rs"]
mod more;
#[path = "failure/recovery.rs"]
mod recovery;
#[path = "failure/round.rs"]
mod round;

fn textual(item: &Item) -> Option<(&str, &str)> {
    match item {
        Item::Message { role, content } => Some((role.as_str(), content.as_str())),
        _ => None,
    }
}

#[derive(Clone, Default)]
struct Faulty {
    requests: Arc<Mutex<Vec<Request>>>,
    failing: Option<String>,
    limit: Option<usize>,
    midstream: Option<String>,
    response: Option<String>,
}

#[async_trait]
impl Provider for Faulty {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("fake-provider"),
            model: "fake-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let attempt = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len()
        };
        if let Some(error) = &self.failing
            && self
                .limit
                .is_none_or(|failure_count| attempt <= failure_count)
        {
            return Err(error.clone());
        }
        if let Some(error) = &self.midstream {
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("partial runtime output".to_string())),
                Err(error.clone()),
            ])));
        }
        if let Some(error) = &self.response {
            return Ok(Box::pin(stream::iter(vec![Ok(Event::Failed(
                error.clone(),
            ))])));
        }
        let events = vec![
            Ok(Event::Text("ok".to_string())),
            Ok(Event::Completed {
                response: Some("fake-response-id".to_string()),
            }),
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}

#[tokio::test]
async fn aggregates() {
    let temp = tempfile::tempdir().expect("temp dir");
    let raw = "openai responses request failed: 401 Unauthorized secret detail".to_string();
    let provider = Arc::new(Faulty {
        failing: Some(raw.clone()),
        ..Faulty::default()
    });
    let service = open(&temp, provider.clone()).await;
    let mut events = service.listen().await;
    let strand = service.weave().await.expect("create strand").strand;
    let first = send(&service, &strand.id, "trigger failure").await;
    let identifier = turn(&service, &first).await.id;
    let incident = identifier.clone();

    let runtime = awaiting(&service, &strand.id, &incident, turn::Status::Failed).await;
    let failed = runtime
        .turns
        .iter()
        .find(|candidate| candidate.id == incident)
        .expect("failed turn");
    assert_eq!(failed.error.as_deref(), Some(raw.as_str()));
    unprojected(&runtime);

    assert_eq!(runtime.errors.len(), 1);
    let identity = runtime.errors[0].id.clone();
    let incident = &runtime.errors[0];
    assert_eq!(incident.code, "provider.turn.failed");
    assert_eq!(incident.status, santi_core::Status::Active);
    assert_eq!(incident.category, Category::Unavailable);
    assert_eq!(incident.occurrences, 1);
    assert_eq!(incident.revision, 1);
    assert_eq!(incident.first.source.component, "santi-provider");
    assert_eq!(incident.first.source.operation, "turn.request");
    assert_eq!(incident.first.context["turn"], identifier);
    assert_eq!(incident.first.context["provider"], "fake-provider");
    assert_eq!(incident.first.context["model"], "fake-model");
    assert_eq!(incident.first.context["stage"], "request");
    assert_eq!(incident.first.context["round"], 1);
    assert!(!incident.exposure.model);

    let event = std::iter::from_fn(|| events.try_recv().ok())
        .find_map(|event| match event.payload {
            santi_core::stream::Payload::Turn(santi_core::turn::Beat::Failed {
                turn: held,
                error,
            }) if held == identifier => Some(error),
            _ => None,
        })
        .expect("turn_failed event");
    assert_eq!(event.code, "provider.turn.failed");
    assert_eq!(event.incident.as_deref(), Some(incident.id.as_str()));
    assert_eq!(event.source.operation, "turn.request");

    let retry = send(&service, &strand.id, "continue after failure").await;

    let runtime = wait(&service, &strand.id, &retry, turn::Status::Failed).await;
    unprojected(&runtime);
    assert_eq!(runtime.errors.len(), 1);
    assert_eq!(runtime.errors[0].id, identity);
    assert_eq!(runtime.errors[0].occurrences, 2);
    assert_eq!(runtime.errors[0].revision, 1);
    assert_eq!(
        runtime.errors[0].latest.context["turn"],
        turn(&service, &retry).await.id
    );
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].input.iter().all(|item| {
        textual(item).is_none_or(|(_, content)| {
            !content.contains("kind: turn_failed") && !content.contains("secret detail")
        })
    }));
}

fn unprojected(runtime: &santi_core::stream::Snapshot) {
    assert!(
        runtime
            .messages
            .iter()
            .all(|message| message.message.kind != message::Kind::System),
        "provider failure must not append a model-visible system message"
    );
}

#[path = "failure/receipt.rs"]
mod receipt;
use receipt::{awaiting, open, send, turn};

async fn wait(
    service: &Service,
    strand: &str,
    posted: &santi_core::strand::Posted,
    status: turn::Status,
) -> santi_core::stream::Snapshot {
    let turn = turn(service, posted).await;
    awaiting(service, strand, &turn.id, status).await
}

async fn aborted(service: &Service, strand: &str, turn: &str) -> santi_core::stream::Snapshot {
    for _ in 0..100 {
        let runtime = service
            .snapshot(strand)
            .await
            .expect("runtime snapshot")
            .expect("strand runtime");
        let failed = runtime
            .turns
            .iter()
            .any(|held| held.id == turn && held.status == turn::Status::Failed);
        let partial = runtime.messages.iter().any(|message| {
            message.message.role == message::Role::Soul
                && message.message.state == message::State::Aborted
        });
        if failed && partial {
            return runtime;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("turn {turn} did not persist aborted output");
}
