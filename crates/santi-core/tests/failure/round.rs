use async_trait::async_trait;
use futures_util::stream;
use santi_core::service::{self, Service};
use santi_core::{message, strand, thinking, turn};
use santi_provider::{
    Call, Event, Metadata, Provider, Receipt, Report, Request, Streaming, Trace, Usage,
};
use serde_json::json;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use tokio::time::{Duration, sleep};

type Script = Vec<Result<Event, String>>;

#[derive(Clone, Default)]
struct Scripted {
    rounds: Arc<Mutex<VecDeque<Script>>>,
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
        let script = self
            .rounds
            .lock()
            .unwrap()
            .pop_front()
            .expect("scripted round");
        Ok(Box::pin(stream::iter(script)))
    }
}

#[tokio::test]
async fn recorded() {
    let temp = tempfile::tempdir().expect("temp dir");
    let arguments = json!({"command": "true"});
    let service = opened(
        &temp,
        vec![
            vec![
                received("req_1"),
                Ok(Event::Started {
                    response: Some("resp_1".to_string()),
                }),
                Ok(Event::Called(Call {
                    response: "resp_1".to_string(),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: "call_1".to_string(),
                    name: "shell".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                reported("completed"),
                Ok(Event::Completed {
                    response: Some("resp_1".to_string()),
                }),
            ],
            vec![
                received("req_2"),
                Ok(Event::Started {
                    response: Some("resp_2".to_string()),
                }),
                Ok(Event::Text("all done".to_string())),
                reported("completed"),
                Ok(Event::Completed {
                    response: Some("resp_2".to_string()),
                }),
            ],
        ],
    )
    .await;
    let runtime = settled(&service, turn::Status::Completed).await;
    let rounds: Vec<_> = runtime
        .thinking
        .iter()
        .map(|span| span.round.clone().expect("round record"))
        .collect();
    assert_eq!(
        rounds,
        vec![
            round("req_1", Some("completed"), 0, &["shell"]),
            round("req_2", Some("completed"), 8, &[]),
        ]
    );
}

#[tokio::test]
async fn torn() {
    let temp = tempfile::tempdir().expect("temp dir");
    let service = opened(
        &temp,
        vec![vec![
            received("req_1"),
            Ok(Event::Started {
                response: Some("resp_1".to_string()),
            }),
            Err("stream torn".to_string()),
        ]],
    )
    .await;
    let runtime = settled(&service, turn::Status::Failed).await;
    let [span] = runtime.thinking.as_slice() else {
        panic!("one thinking span: {:?}", runtime.thinking);
    };
    assert_eq!(span.state, thinking::State::Failed);
    assert_eq!(span.error.as_deref(), Some("stream torn"));
    let mut expected = round("req_1", None, 0, &[]);
    expected.model = None;
    expected.usage = None;
    assert_eq!(span.round, Some(expected));
}

fn received(request: &str) -> Result<Event, String> {
    Ok(Event::Traced(Trace::Received(Receipt {
        status: 200,
        elapsed: 42,
        headers: vec![("x-request-id".to_string(), request.to_string())],
    })))
}

fn reported(status: &str) -> Result<Event, String> {
    Ok(Event::Traced(Trace::Reported(Report {
        status: Some(status.to_string()),
        incomplete: None,
        model: Some("gpt-5.5".to_string()),
        usage: Some(Usage {
            input: Some(100),
            cached: Some(80),
            output: Some(10),
            reasoning: Some(4),
        }),
    })))
}

fn round(request: &str, status: Option<&str>, text: usize, calls: &[&str]) -> thinking::Round {
    thinking::Round {
        code: Some(200),
        millis: Some(42),
        headers: BTreeMap::from([("x-request-id".to_string(), request.to_string())]),
        status: status.map(str::to_string),
        incomplete: None,
        model: Some("gpt-5.5".to_string()),
        usage: Some(Usage {
            input: Some(100),
            cached: Some(80),
            output: Some(10),
            reasoning: Some(4),
        }),
        text,
        calls: calls.iter().map(|name| name.to_string()).collect(),
        continuation: None,
    }
}

async fn opened(temp: &tempfile::TempDir, rounds: Vec<Script>) -> Service {
    santi_core::Store::bootstrap(
        temp.path().join("santi.sqlite"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .expect("bootstrap");
    let provider = Scripted {
        rounds: Arc::new(Mutex::new(rounds.into())),
    };
    Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(provider),
    )
    .await
    .expect("open service")
}

async fn settled(service: &Service, status: turn::Status) -> santi_core::stream::Snapshot {
    let strand = service.weave().await.expect("create strand").strand;
    service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "run something".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    for _ in 0..100 {
        let runtime = service
            .snapshot(&strand.id)
            .await
            .expect("runtime snapshot")
            .expect("strand runtime");
        if runtime.turns.iter().any(|held| held.status == status) {
            return runtime;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("turn did not settle as {status:?}");
}
