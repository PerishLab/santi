use super::*;
use santi_estate::{CallDraft, ForkDraft, ReplyDraft, ThinkingDraft, TurnDraft};

#[tokio::test]
async fn active() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let provider = Arc::new(Script::new(vec![
        tools(),
        finish("settled"),
        finish("child"),
    ]));
    let service = Service::open(config(&temp), provider.clone())
        .await
        .unwrap();
    let parent = service.weave().await.unwrap().strand;
    send(&service, &parent.id).await;
    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    let head = store.strand(&parent.id).await.unwrap().unwrap().next - 1;
    opened(&store, &parent.id, head).await;
    let partial = store.strand(&parent.id).await.unwrap().unwrap().next - 1;
    let broken = store
        .fork(ForkDraft {
            tag: "ss_partial",
            parent: &parent.id,
            at: partial,
            created: &santi_core::now(),
        })
        .await
        .unwrap();
    rejected(&service, &broken.id).await;
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    let child = service.fork(&parent.id).await.unwrap().strand;
    assert_eq!(child.fork, Some(head));
    send(&service, &child.id).await;
    assert_eq!(
        originals(&provider.requests.lock().unwrap()[2].input),
        ["original 工具🙂", "original settled"]
    );
    closed(&store, &parent.id).await;
    let repaired = service.fork(&broken.id).await.unwrap().strand;
    assert_eq!(repaired.fork, Some(head));
    let inherited = santi_core::provider_input(&store, &repaired.id)
        .await
        .unwrap();
    assert!(
        !inherited
            .iter()
            .any(|item| matches!(item, Item::Call {call,..} if call == "call_active"))
    );
    let settled = service.fork(&parent.id).await.unwrap().strand;
    assert_eq!(settled.fork, Some(partial + 3));
}

async fn rejected(service: &Service, strand: &str) {
    let posted = service
        .send(
            strand,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "inspect the fork".into(),
                }],
            },
        )
        .await
        .unwrap();
    let failed = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let status = service
                .receipt(&posted.receipt.inbox)
                .await
                .unwrap()
                .unwrap();
            if status.state == santi_core::receipt::State::Failed {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    assert!(
        failed.is_ok(),
        "broken history must settle its receipt as failed"
    );
    let snapshot = service.snapshot(strand).await.unwrap().unwrap();
    assert!(snapshot.errors.iter().any(|incident| {
        incident.code == "runtime.history.invalid"
            && incident.category == santi_core::error::Category::Invalid
            && incident.retry == santi_core::error::Retry::Changed
    }));
}

async fn closed(store: &santi_core::Store, parent: &str) {
    let now = santi_core::now();
    fixture(async || {
        if let Some(call) = store.call("call_second").await? {
            assert_eq!(call.turn, "turn_active");
            assert_eq!(call.tool, "shell");
            assert_eq!(call.arguments, json!({"command":"printf second"}));
            return Ok(());
        }
        store
            .create_call(CallDraft {
                tag: "call_second",
                turn: "turn_active",
                tool: "shell",
                arguments: &json!({"command":"printf second"}),
                created: &now,
            })
            .await
            .map(|_| ())
    })
    .await;
    for (tag, call) in [
        ("reply_active", "call_active"),
        ("reply_second", "call_second"),
    ] {
        replied(store, tag, call, &now).await;
    }
    let head = store.strand(parent).await.unwrap().unwrap().next - 1;
    fixture(async || {
        let turn = store.turn("turn_active").await?.expect("fixture turn");
        if turn.status == santi_core::turn::Status::Completed {
            assert_eq!(turn.to, Some(head));
            return Ok(());
        }
        store
            .complete_turn("turn_active", head, &now)
            .await
            .map(|_| ())
    })
    .await;
}

async fn replied(store: &santi_core::Store, tag: &str, call: &str, now: &str) {
    fixture(async || {
        if let Some(reply) = store.reply(tag).await? {
            assert_eq!(reply.call, call);
            assert_eq!(reply.output, Some(json!({"exit_code":0})));
            assert!(reply.error.is_none());
            return Ok(());
        }
        store
            .create_reply(ReplyDraft {
                tag,
                call,
                output: Some(&json!({"exit_code":0})),
                error: None,
                created: now,
            })
            .await
            .map(|_| ())
    })
    .await;
}

async fn fixture(mut write: impl AsyncFnMut() -> Result<(), String>) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let result = tokio::time::timeout_at(deadline, write())
            .await
            .expect("fixture writer deadline");
        match result {
            Ok(()) => return,
            Err(error)
                if error == "store unavailable" && tokio::time::Instant::now() < deadline =>
            {
                sleep(Duration::from_millis(10)).await;
            }
            Err(error) => panic!("fixture write failed: {error}"),
        }
    }
}

async fn opened(store: &santi_core::Store, parent: &str, head: i64) {
    let now = santi_core::now();
    store
        .create_turn(TurnDraft {
            tag: "turn_active",
            strand: parent,
            trigger: santi_core::turn::Trigger::StrandSend,
            from: head + 1,
            created: &now,
        })
        .await
        .unwrap();
    store
        .create_thinking(ThinkingDraft {
            tag: "thinking_active",
            turn: "turn_active",
            response: Some("active"),
            created: &now,
        })
        .await
        .unwrap();
    store
        .chronicle(
            "thinking_active",
            &thinking::Round {
                calls: vec!["shell".into(), "shell".into()],
                continuation: Some(Box::new(state("unfinished", 2))),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    store
        .create_call(CallDraft {
            tag: "call_active",
            turn: "turn_active",
            tool: "shell",
            arguments: &json!({"command":"santi strand fork"}),
            created: &now,
        })
        .await
        .unwrap();
}
