use super::super::support::*;
use santi_core::service::{self, Service};
use santi_core::{Store, message};
use santi_provider::{Call, Tool};
use serde_json::json;

struct Recovery {
    requests: Mutex<Vec<Request>>,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[async_trait]
impl Provider for Recovery {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("recovery"),
            model: "recovery".into(),
            budget: Some(santi_provider::Cap {
                bytes: 200_000,
                maintenance: 1_000_000,
                source: "test".into(),
            }),
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let compact = matches!(request.tools.as_deref(), Some([Tool::Function(tool)]) if tool.name == "compact");
        self.requests.lock().unwrap().push(request);
        if compact {
            self.entered.notify_one();
            self.release.notified().await;
            let arguments = json!({"summary":"Keep issue #96 acceptance and live Job job_kept; original history remains queryable."});
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Called(Call {
                    response: "recovery".into(),
                    mark: None,
                    item: json!({"type":"function_call"}),
                    call: "call_recovery".into(),
                    name: "compact".into(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                Ok(Event::Completed {
                    response: Some("recovery".into()),
                }),
            ])));
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("resumed".into())),
            Ok(Event::Completed {
                response: Some("resumed".into()),
            }),
        ])))
    }
}

#[tokio::test]
async fn pending() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let constitution = temp.path().join("constitution");
    std::fs::write(&constitution, "instruction".repeat(7000)).unwrap();
    let config = service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: Some(constitution.display().to_string()),
        environment: Default::default(),
    };
    let provider = Arc::new(Recovery {
        requests: Mutex::new(Vec::new()),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let service = Service::open(config.clone(), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    let store = Store::open(&config.database).await.unwrap();
    for (tag, text) in [
        ("msg_history", "x".repeat(160_000)),
        ("msg_recent", "issue #96 acceptance; job_kept".into()),
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
    store
        .accept_inbox(
            santi_core::InboxDraft {
                tag: "inbox_kept",
                strand: &strand.id,
                kind: message::Kind::Text,
                content: &message::Content::text("pending external input"),
                source: None,
                created: &santi_core::now(),
            },
            500,
        )
        .await
        .unwrap();
    service.resume().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), provider.entered.notified())
        .await
        .unwrap();
    assert!(store.inbox("inbox_kept").await.unwrap().is_some());
    assert!(
        store
            .receipt("inbox_kept")
            .await
            .unwrap()
            .unwrap()
            .transitions
            .iter()
            .all(|transition| transition.turn.is_none())
    );
    {
        let requests = provider.requests.lock().unwrap();
        assert!(requests[0].input.iter().all(|item| !matches!(item, Item::Message { content, .. } if content.contains("pending external input"))));
        assert!(requests[0].instructions.as_ref().unwrap().len() > 70_000);
    }
    provider.release.notify_one();
    Probe::new(&service).any_completed(&strand.id).await;
    service.resume().await.unwrap();
    for _ in 0..100 {
        if store.receipt("inbox_kept").await.unwrap().unwrap().state
            == santi_core::receipt::State::Completed
        {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        store.receipt("inbox_kept").await.unwrap().unwrap().state,
        santi_core::receipt::State::Completed
    );
    assert_eq!(
        store
            .message("msg_history")
            .await
            .unwrap()
            .unwrap()
            .message
            .content
            .rendered()
            .len(),
        160_000
    );
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1].input.iter().any(
            |item| matches!(item, Item::Message { content, .. } if content.contains("job_kept"))
        )
    );
    assert!(requests[1].input.iter().any(|item| matches!(item, Item::Message { content, .. } if content.contains("pending external input"))));
}
