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
        let compact = matches!(request.tools.as_deref(), Some(tools) if tools.iter().any(|Tool::Function(tool)| tool.name == "compact"));
        let round = self.requests.lock().unwrap().len();
        let slots = request.input.iter().rev().find_map(|item| match item {
            Item::Output { output, .. } => serde_json::from_str::<serde_json::Value>(output)
                .ok()
                .and_then(|value| value["output"]["slots"].as_array().cloned())
                .map(serde_json::Value::Array),
            _ => None,
        });
        self.requests.lock().unwrap().push(request);
        if compact {
            if round == 0 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            let (name, arguments) = choice(round, slots);
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Called(Call {
                    response: "recovery".into(),
                    mark: None,
                    item: json!({"type":"function_call"}),
                    call: format!("call_recovery_{round}"),
                    name: name.into(),
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
    let mut events = service.listen().await;
    for index in 0..5 {
        let tag = format!("msg_slot_{index}");
        store
            .place(santi_estate::MessageDraft {
                tag: &tag,
                strand: &strand.id,
                actor: message::Role::System,
                actor_id: santi_core::SYSTEM,
                kind: message::Kind::Text,
                content: &message::Content::text("prior memory"),
                state: message::State::Fixed,
                request: false,
                created: &santi_core::now(),
            })
            .await
            .unwrap();
        store
            .create_compact(santi_estate::CompactDraft {
                tag: &format!("compact_slot_{index}"),
                strand: &strand.id,
                first: &tag,
                last: &tag,
                summary: "prior memory; live Job job_kept",
                metadata: None,
                expected: None,
                created: &santi_core::now(),
            })
            .await
            .unwrap();
    }
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
    let posted = service
        .send(
            &strand.id,
            santi_core::strand::Post {
                content: vec![message::Part::Text {
                    text: "pending external input".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert!(posted.turn.is_none());
    assert!(posted.message.is_none());
    let accepted = store.receipt(&posted.receipt.inbox).await.unwrap().unwrap();
    assert_eq!(accepted.state, santi_core::receipt::State::Accepted);
    assert!(
        accepted
            .transitions
            .iter()
            .all(|transition| transition.turn.is_none())
    );
    tokio::time::timeout(Duration::from_secs(5), provider.entered.notified())
        .await
        .unwrap();
    assert!(store.inbox(&posted.receipt.inbox).await.unwrap().is_some());
    assert!(
        store
            .receipt(&posted.receipt.inbox)
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
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = events.recv().await.expect("runtime progress event");
            if event.strand == strand.id
                && matches!(
                    event.payload,
                    santi_core::stream::Payload::Turn(santi_core::turn::Beat::Completed { .. })
                )
            {
                break;
            }
        }
    })
    .await
    .expect("maintenance turn completes");
    service.resume().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if store
                .receipt(&posted.receipt.inbox)
                .await
                .unwrap()
                .unwrap()
                .state
                == santi_core::receipt::State::Completed
            {
                break;
            }
            events.recv().await.expect("runtime progress event");
        }
    })
    .await
    .expect("pending input completes after maintenance");
    assert_eq!(
        store
            .receipt(&posted.receipt.inbox)
            .await
            .unwrap()
            .unwrap()
            .state,
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
    let resumed = requests.iter().find(|request| {
        let maintenance = matches!(request.tools.as_deref(), Some(tools) if tools.iter().any(|Tool::Function(tool)| tool.name == "compact"));
        !maintenance && request.input.iter().any(|item| matches!(item, Item::Message { content, .. } if content.contains("pending external input")))
    }).expect("ordinary request consumes the pending input");
    assert!(requests[1].input.iter().any(|item| matches!(item, Item::Output { output, .. } if output.contains("query slots or messages"))));
    assert!(
        resumed.input.iter().any(
            |item| matches!(item, Item::Message { content, .. } if content.contains("job_kept"))
        )
    );
    assert!(resumed.input.iter().any(|item| matches!(item, Item::Message { content, .. } if content.contains("pending external input"))));
}

fn choice(round: usize, slots: Option<serde_json::Value>) -> (&'static str, serde_json::Value) {
    match round {
        0 => (
            "compact",
            json!({"summary":"retry with current references", "first":"missing", "last":"missing"}),
        ),
        1 => ("query", json!({})),
        2 => merged(slots),
        3 => (
            "compact",
            json!({"summary":"Keep issue #96 acceptance and live Job job_kept; original history remains queryable."}),
        ),
        _ if round.is_multiple_of(2) => ("query", json!({})),
        _ => merged(slots),
    }
}

fn merged(slots: Option<serde_json::Value>) -> (&'static str, serde_json::Value) {
    let slots = slots.expect("current slots from query");
    (
        "compact",
        json!({"summary":"Keep both prior memories and Job job_kept", "absorb":[slots[0]["id"],slots[1]["id"]]}),
    )
}
