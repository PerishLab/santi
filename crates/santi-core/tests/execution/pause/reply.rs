use super::super::super::support::*;
use santi_core::{Store, message, service, strand};
use santi_provider::Call;
use serde_json::json;

struct Speaker {
    replies: usize,
    requests: Mutex<Vec<Request>>,
}

#[async_trait]
impl Provider for Speaker {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("speaker"),
            model: "speaker".into(),
            budget: Some(santi_provider::Cap {
                bytes: 200_000,
                maintenance: 400_000,
                source: "test".into(),
            }),
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let maintaining = request
            .tools
            .as_ref()
            .unwrap()
            .iter()
            .any(|santi_provider::Tool::Function(tool)| tool.name == "compact");
        let count = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len() - 1
        };
        if !maintaining {
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("resumed".into())),
                Ok(Event::Completed { response: None }),
            ])));
        }
        if count < self.replies {
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text(format!("report {count}"))),
                Ok(Event::Completed { response: None }),
            ])));
        }
        let arguments = json!({"summary": "Settled history before the recent message."});
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Called(Call {
                response: "compact".into(),
                mark: None,
                item: json!({"type":"function_call"}),
                call: format!("call_{count}"),
                name: "compact".into(),
                raw: arguments.to_string(),
                arguments,
            })),
            Ok(Event::Completed { response: None }),
        ])))
    }
}

struct Spoken {
    _temp: tempfile::TempDir,
    provider: Arc<Speaker>,
    service: service::Service,
    strand: String,
}

async fn spoken(replies: usize) -> Spoken {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let config = service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    };
    let provider = Arc::new(Speaker {
        replies,
        requests: Mutex::new(Vec::new()),
    });
    let service = service::Service::open(config.clone(), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    let store = Store::open(&config.database).await.unwrap();
    for (tag, text) in [
        ("msg_history", "x".repeat(230_000)),
        ("msg_recent", "recent".into()),
    ] {
        store
            .place(santi_estate::MessageDraft {
                tag,
                strand: &strand.id,
                actor: message::Role::System,
                actor_id: santi_core::SYSTEM,
                kind: message::Kind::Text,
                content: &message::Content::text(text),
                state: message::State::Fixed,
                request: false,
                created: &santi_core::now(),
            })
            .await
            .unwrap();
    }
    service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "pending".into(),
                }],
            },
        )
        .await
        .unwrap();
    Spoken {
        _temp: temp,
        provider,
        service,
        strand: strand.id,
    }
}

async fn ended(service: &service::Service, strand: &str) -> santi_core::stream::Snapshot {
    for _ in 0..300 {
        let runtime = service.snapshot(strand).await.unwrap().unwrap();
        if runtime.turns.iter().any(|turn| {
            matches!(
                turn.status,
                santi_core::turn::Status::Completed | santi_core::turn::Status::Failed
            )
        }) {
            return runtime;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("no turn ended");
}

fn position(request: &Request, needle: &str) -> Option<usize> {
    request
        .input
        .iter()
        .position(|item| matches!(item, Item::Message { content, .. } if content.contains(needle)))
}

#[tokio::test]
async fn replies() {
    let Spoken {
        _temp,
        provider,
        service,
        strand,
    } = spoken(2).await;
    let runtime = ended(&service, &strand).await;
    assert!(
        runtime
            .turns
            .iter()
            .all(|turn| turn.status != santi_core::turn::Status::Failed)
    );
    assert_eq!(runtime.compacts.len(), 1);
    for text in ["report 0", "report 1"] {
        assert!(runtime.messages.iter().any(|message| message.text == text));
    }
    let requests = provider.requests.lock().unwrap();
    let settlement = "kind: context_maintenance\nstate: Query and compact are available.";
    for (index, request) in requests.iter().take(3).enumerate() {
        let held = position(request, settlement).expect("maintenance settlement");
        let Item::Message { content, .. } = &request.input[held] else {
            unreachable!()
        };
        assert!(content.contains("a reply without a tool call does not end it"));
        if index > 0 {
            let reply = position(request, &format!("report {}", index - 1)).expect("reply");
            assert!(reply < held);
        }
        assert!(position(request, "paused for context maintenance").is_some());
        assert!(position(request, "Only compact is available").is_none());
    }
}

#[tokio::test]
async fn exhausts() {
    let Spoken {
        _temp,
        provider,
        service,
        strand,
    } = spoken(usize::MAX).await;
    let runtime = ended(&service, &strand).await;
    assert!(runtime.compacts.is_empty());
    assert!(
        runtime
            .turns
            .iter()
            .any(|turn| turn.status == santi_core::turn::Status::Failed)
    );
    let incident = service
        .stranded(&strand, 10)
        .await
        .unwrap()
        .unwrap()
        .into_iter()
        .find(|incident| incident.code == "context.budget.exceeded")
        .unwrap();
    assert_eq!(incident.latest.context["reason"], "compact_required");
    assert_eq!(provider.requests.lock().unwrap().len(), 3);
}
