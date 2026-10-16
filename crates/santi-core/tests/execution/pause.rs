use super::super::support::*;
use santi_core::{Store, message, service, strand};
use santi_provider::Call;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Reader {
    bytes: AtomicUsize,
    requests: Mutex<Vec<Request>>,
}

#[async_trait]
impl Provider for Reader {
    fn metadata(&self) -> Metadata {
        let bytes = self.bytes.load(Ordering::SeqCst);
        Metadata {
            provider: Arc::from("reader"),
            model: "reader".into(),
            budget: Some(santi_provider::Cap {
                bytes,
                maintenance: bytes * 2,
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
        let count = self.requests.lock().unwrap().len();
        self.requests.lock().unwrap().push(request);
        if !maintaining {
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("resumed".into())),
                Ok(Event::Completed { response: None }),
            ])));
        }
        let arguments = json!({"kind":"content", "id":"msg_history"});
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Called(Call {
                response: "query".into(),
                mark: None,
                item: json!({"type":"function_call"}),
                call: format!("call_{count}"),
                name: "query".into(),
                raw: arguments.to_string(),
                arguments,
            })),
            Ok(Event::Completed { response: None }),
        ])))
    }
}

#[tokio::test]
async fn holds() {
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
    let provider = Arc::new(Reader {
        bytes: AtomicUsize::new(200_000),
        requests: Mutex::new(Vec::new()),
    });
    let service = service::Service::open(config.clone(), provider.clone())
        .await
        .unwrap()
        .bounded(santi_core::budget::Execution {
            profile: "test".into(),
            rounds: 1,
            calls: 1,
            output: 16,
            shell: 16,
            feedback_after_calls: None,
            feedback_command: None,
            feedback_cwd: None,
        })
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
    let posted = service
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
    for _ in 0..300 {
        let runtime = service.snapshot(&strand.id).await.unwrap().unwrap();
        if runtime
            .turns
            .iter()
            .any(|turn| turn.status == santi_core::turn::Status::Failed)
        {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    let incident = service
        .stranded(&strand.id, 10)
        .await
        .unwrap()
        .unwrap()
        .into_iter()
        .find(|incident| incident.code == "context.budget.exceeded")
        .unwrap();
    assert_eq!(
        incident.latest.context["details"]["maintenance_pause"],
        true
    );
    let count = provider.requests.lock().unwrap().len();
    assert!(count > 1 && count < 30);
    {
        let requests = provider.requests.lock().unwrap();
        for request in requests.iter() {
            let Item::Message { content, .. } = request.input.last().unwrap() else {
                panic!("missing suffix")
            };
            assert!(content.contains("maintenance_bytes: 00000000000000300000"));
            assert!(content.contains("remaining_bytes:"));
        }
    }
    let queued = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "still pending".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert!(queued.turn.is_none());
    assert_eq!(
        store
            .receipt(&posted.receipt.inbox)
            .await
            .unwrap()
            .unwrap()
            .state,
        santi_core::receipt::State::Accepted
    );
    service.resume().await.unwrap();
    assert_eq!(provider.requests.lock().unwrap().len(), count);
    service.close();
    drop(service);
    provider.bytes.store(1_000_000, Ordering::SeqCst);
    let service = service::Service::open(config, provider.clone())
        .await
        .unwrap();
    service.resume().await.unwrap();
    assert_eq!(provider.requests.lock().unwrap().len(), count);
    service.drive(&strand.id).await.unwrap();
    Probe::new(&service).any_completed(&strand.id).await;
    assert_eq!(provider.requests.lock().unwrap().len(), count + 1);
    assert!(store.message("msg_history").await.unwrap().is_some());
}
