mod fork;
mod pressure;
mod preview;

use super::super::support::*;
use santi_core::service::{self, Service};
use santi_core::{compact, message, strand, thinking};
use santi_provider::Call;
use santi_provider::chat::completions::{Chat, Config, Replay};
use serde_json::json;
use std::collections::VecDeque;

struct Script {
    chat: Chat,
    limit: Option<usize>,
    scripts: Mutex<VecDeque<Vec<Event>>>,
    requests: Mutex<Vec<Request>>,
}

impl Script {
    fn new(scripts: Vec<Vec<Event>>) -> Self {
        Self {
            limit: None,
            chat: Chat::new(Config {
                provider: "deepseek".into(),
                key: "fixture".into(),
                model: "flash".into(),
                url: "http://provider.test".into(),
                thinking: Some("enabled".into()),
                replay: Replay::History,
                effort: None,
                ceiling: None,
                bytes: None,
                maintenance: None,
                headers: Vec::new(),
            }),
            scripts: Mutex::new(scripts.into()),
            requests: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl Provider for Script {
    fn metadata(&self) -> Metadata {
        let mut metadata = self.chat.metadata();
        metadata.budget = self.limit.map(|bytes| santi_provider::Cap {
            bytes,
            maintenance: bytes + bytes / 2,
            source: "test".into(),
        });
        metadata
    }
    fn prepare(&self, request: Request) -> Result<Request, String> {
        self.chat.prepare(request)
    }
    fn validate(&self, request: &Request) -> Result<(), String> {
        self.chat.validate(request)
    }
    fn estimate(&self, request: &Request) -> Option<santi_core::budget::Estimate> {
        self.chat.estimate(request)
    }
    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        self.chat.validate(&request)?;
        self.requests.lock().unwrap().push(request);
        let script = self
            .scripts
            .lock()
            .unwrap()
            .pop_front()
            .expect("next scripted response");
        Ok(Box::pin(stream::iter(script.into_iter().map(Ok))))
    }
}

fn state(content: &str, calls: usize) -> thinking::Continuation {
    thinking::Continuation {
        provider: "deepseek".into(),
        model: "flash".into(),
        endpoint: "http://provider.test".into(),
        content: Some(content.into()),
        calls,
    }
}

fn finish(id: &str) -> Vec<Event> {
    vec![
        Event::Started {
            response: Some(id.into()),
        },
        Event::Thinking("display summary".into()),
        Event::Retained(state(&format!("original {id}"), 0)),
        Event::Text("done".into()),
        Event::Completed {
            response: Some(id.into()),
        },
    ]
}

fn tools() -> Vec<Event> {
    let mut events = vec![
        Event::Started {
            response: Some("tools".into()),
        },
        Event::Thinking("display summary".into()),
        Event::Text("checking".into()),
        Event::Retained(state("original 工具🙂", 2)),
    ];
    for name in ["one", "two"] {
        let arguments = json!({"command":format!("printf {name}")});
        events.push(Event::Called(Call {
            response: "tools".into(),
            mark: None,
            item: json!({"type":"function_call"}),
            call: name.into(),
            name: "shell".into(),
            raw: arguments.to_string(),
            arguments,
        }));
    }
    events
}

fn config(temp: &tempfile::TempDir) -> service::Config {
    service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    }
}

async fn send(service: &Service, id: &str) {
    let posted = service
        .send(
            id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "continue".into(),
                }],
            },
        )
        .await
        .unwrap();
    let turn = accepted_turn(service, &posted).await;
    Probe::new(service).completed_turn(id, &turn.id).await;
}

fn originals(input: &[Item]) -> Vec<String> {
    input
        .iter()
        .filter_map(|item| match item {
            Item::Continuation(state) => state.content.clone(),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn restores() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let provider = Arc::new(Script::new(vec![tools(), finish("answer")]));
    let service = Service::open(config(&temp), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    assert_eq!(
        originals(&provider.requests.lock().unwrap()[1].input),
        ["original 工具🙂"]
    );
    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    let spans = store.thinkings(&strand.id).await.unwrap();
    let saved = spans
        .iter()
        .find(|span| {
            span.round.as_ref().is_some_and(|round| {
                round
                    .continuation
                    .as_ref()
                    .is_some_and(|state| state.calls == 2)
            })
        })
        .unwrap();
    assert_eq!(saved.summary.as_deref(), Some("display summary"));
    assert_eq!(
        saved
            .round
            .as_ref()
            .unwrap()
            .continuation
            .as_ref()
            .unwrap()
            .content
            .as_deref(),
        Some("original 工具🙂")
    );
    drop(service);
    let next = Arc::new(Script::new(vec![finish("followup")]));
    let service = Service::open(config(&temp), next.clone()).await.unwrap();
    send(&service, &strand.id).await;
    assert_eq!(
        originals(&next.requests.lock().unwrap()[0].input),
        ["original 工具🙂", "original answer"]
    );
    let child = service.fork(&strand.id).await.unwrap().strand;
    let inherited = santi_core::provider_input(&store, &child.id).await.unwrap();
    assert_eq!(
        originals(&inherited),
        ["original 工具🙂", "original answer", "original followup"]
    );
    let request = next
        .prepare(Request {
            model: "flash".into(),
            instructions: None,
            input: inherited,
            tools: Some(vec![santi_provider::Tool::Function(
                santi_provider::Function {
                    name: "shell".into(),
                    description: "shell".into(),
                    parameters: json!({}),
                },
            )]),
            previous: None,
        })
        .unwrap();
    next.validate(&request).unwrap();
}

#[tokio::test]
async fn compacts() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let provider = Arc::new(Script::new(vec![
        tools(),
        finish("answer"),
        finish("followup"),
    ]));
    let service = Service::open(config(&temp), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let detail = service.strand(&strand.id).await.unwrap().unwrap();
    let first = detail.messages.first().unwrap().message.id.clone();
    let last = detail.messages.last().unwrap().message.id.clone();
    let report = service
        .exec(
            &strand.id,
            compact::Exec {
                first: Some(first),
                last: Some(last),
                from: None,
                to: None,
                summary: "completed probe".into(),
                absorb: Vec::new(),
                capsule: None,
                dry: false,
            },
        )
        .await
        .unwrap();
    assert!(report.after.as_ref().unwrap().total < report.before.as_ref().unwrap().total);
    send(&service, &strand.id).await;
    let requests = provider.requests.lock().unwrap();
    assert!(
        originals(&requests[2].input).is_empty(),
        "compact must not reload archived reasoning"
    );
    assert!(requests[2].input.iter().any(
        |item| matches!(item, Item::Message {content,..} if content.contains("completed probe"))
    ));
}
