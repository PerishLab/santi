use async_trait::async_trait;
use futures_util::stream;
use santi_core::service::{self, Service};
use santi_core::{message, strand, turn};
use santi_provider::{Call, Event, Metadata, Provider, Receipt, Report, Request, Streaming, Trace};
use serde_json::json;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
use tokio::time::{Duration, sleep};

type Script = Vec<Result<Event, String>>;

#[derive(Clone, Default)]
struct Scripted {
    rounds: Arc<Mutex<VecDeque<Script>>>,
    requests: Arc<Mutex<usize>>,
    gate: Option<Arc<Notify>>,
}

#[async_trait]
impl Provider for Scripted {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("scripted-provider"),
            model: "scripted-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, _: Request) -> Result<Streaming, String> {
        if let Some(gate) = &self.gate {
            gate.notified().await;
        }
        *self.requests.lock().unwrap() += 1;
        let script = self
            .rounds
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted round");
        Ok(Box::pin(stream::iter(script)))
    }
}

async fn open(provider: Arc<Scripted>) -> (tempfile::TempDir, Service) {
    let temp = tempfile::tempdir().expect("temp dir");
    let database = temp.path().join("santi.sqlite");
    santi_core::Store::bootstrap(
        database.clone(),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("bootstrap");
    let opened = Service::open(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        provider,
    )
    .await
    .expect("open service");
    (temp, opened)
}

fn received() -> Result<Event, String> {
    Ok(Event::Traced(Trace::Received(Receipt {
        status: 200,
        elapsed: 1,
        headers: vec![],
    })))
}

fn reported(status: &str) -> Result<Event, String> {
    Ok(Event::Traced(Trace::Reported(Report {
        status: Some(status.to_string()),
        incomplete: None,
        model: Some("scripted-model".to_string()),
        usage: None,
    })))
}

fn started(response: &str) -> Result<Event, String> {
    Ok(Event::Started {
        response: Some(response.to_string()),
    })
}

fn completed(response: &str) -> Result<Event, String> {
    Ok(Event::Completed {
        response: Some(response.to_string()),
    })
}

struct Observation {
    status: turn::Status,
    rounds: Vec<Option<String>>,
    finish: Option<Option<String>>,
    replies: usize,
    requests: usize,
    texts: Vec<String>,
    running: bool,
}

async fn run(rounds: Vec<Script>, gate: Option<Arc<Notify>>) -> Observation {
    let provider = Arc::new(Scripted {
        rounds: Arc::new(Mutex::new(rounds.into())),
        gate: gate.clone(),
        ..Scripted::default()
    });
    let requests = provider.requests.clone();
    let (_temp, service) = open(provider).await;
    let strand = service.weave().await.expect("weave").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "truncation probe".to_string(),
                }],
            },
        )
        .await
        .expect("send");
    let mut running = false;
    let settled = loop {
        let snapshot = service
            .snapshot(&strand.id)
            .await
            .expect("snapshot")
            .expect("strand");
        if let Some(current) = snapshot
            .turns
            .last()
            .filter(|held| held.status == turn::Status::Running)
        {
            running = true;
            assert!(
                current.finish.is_none(),
                "running turn must not project a terminal finish"
            );
            if let Some(gate) = &gate {
                gate.notify_one();
            }
        }
        if snapshot
            .turns
            .iter()
            .any(|held| held.status != turn::Status::Running)
        {
            break snapshot;
        }
        sleep(Duration::from_millis(10)).await;
    };
    let mut done = None;
    for _ in 0..200 {
        let receipt = service
            .receipt(&posted.receipt.inbox)
            .await
            .expect("receipt")
            .expect("accepted");
        if format!("{:?}", receipt.state) == "Completed" {
            done = Some(receipt);
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    let receipt = done.expect("completed receipt");
    let runtime = service
        .snapshot(&strand.id)
        .await
        .expect("snapshot")
        .expect("strand");
    let held = settled.turns.last().expect("turn").clone();
    let rounds: Vec<Option<String>> = runtime
        .thinking
        .iter()
        .map(|span| span.round.as_ref().and_then(|round| round.status.clone()))
        .collect();
    let replies = runtime
        .messages
        .iter()
        .filter(|placed| placed.message.role == message::Role::Soul)
        .count();
    let texts: Vec<String> = runtime
        .messages
        .iter()
        .filter(|placed| placed.message.role == message::Role::Soul)
        .map(|placed| placed.message.content.rendered())
        .collect();
    let finish = held.finish.as_ref().map(|fact| fact.status.clone());
    if held.finish.is_some() {
        let mut value = serde_json::to_value(&held).expect("turn serializes");
        assert!(value.get("finish").is_some(), "settled turn exposes finish");
        value.as_object_mut().expect("turn object").remove("finish");
        let old: turn::Turn = serde_json::from_value(value).expect("old record stays readable");
        assert!(old.finish.is_none(), "missing finish stays unknown");
    }
    let state = format!("{:?}", receipt.state);
    assert_eq!(state, "Completed", "receipt terminal state");
    assert_eq!(held.status, turn::Status::Completed, "turn terminal state");
    let requests = *requests.lock().unwrap();
    println!(
        "receipt={state} rounds={rounds:?} finish={finish:?} replies={replies} requests={requests}"
    );
    Observation {
        status: held.status,
        rounds,
        finish,
        replies,
        requests,
        texts,
        running,
    }
}

#[path = "finish_turn/more.rs"]
mod more;
