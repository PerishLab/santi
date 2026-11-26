use super::*;
use santi_estate::{Call, Reply, Thinking};

#[tokio::test]
async fn active() {
    for (dry, capsule) in [(true, false), (true, true), (false, false), (false, true)] {
        exercise(dry, capsule).await;
    }
}

async fn exercise(dry: bool, capsule: bool) {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let mut provider = Script::new(vec![tools(), finish("answer"), finish("resumed")]);
    provider.limit = Some(500_000);
    let provider = Arc::new(provider);
    let service = Service::open(crate::SCENARIO, config(&temp), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let messages = service.strand(&strand.id).await.unwrap().unwrap().messages;
    let first = messages.first().unwrap().message.id.clone();
    let last = messages.last().unwrap().message.id.clone();
    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    opened(&store, &strand.id).await;

    let refused = service
        .exec(
            &strand.id,
            request(&first, &messages[1].message.id, true, false),
        )
        .await
        .expect_err("a compact cannot orphan retained tool calls");
    assert!(
        refused.contains("compact.exchange") || refused.contains("compact.anchor"),
        "{refused}"
    );
    assert!(store.compacts(&strand.id).await.unwrap().is_empty());

    let report = service
        .exec(&strand.id, request(&first, &last, dry, capsule))
        .await
        .expect("compact earlier history with an unfinished tool exchange");
    assert_eq!(report.dry, dry);
    assert_eq!(
        store.compacts(&strand.id).await.unwrap().len(),
        usize::from(!dry)
    );
    let retained = santi_core::assemble(&store, &strand.id).await.unwrap();
    assert!(originals(&retained).contains(&"active reasoning".into()));
    assert_eq!(
        originals(&retained).contains(&"original answer".into()),
        dry
    );
    assert!(
        retained
            .iter()
            .any(|item| { matches!(item, Item::Call {call, ..} if call == "call_active") })
    );
    if let Some(after) = &report.after {
        assert!(after.total > 0);
    }

    closed(&store, &strand.id).await;
    send(&service, &strand.id).await;
    let requests = provider.requests.lock().unwrap();
    let resumed = requests.last().unwrap();
    assert!(originals(&resumed.input).contains(&"active reasoning".into()));
    for call in ["call_active", "call_second"] {
        assert!(
            resumed
                .input
                .iter()
                .any(|item| { matches!(item, Item::Output {call: held, ..} if held == call) })
        );
    }
    let Some(Item::Message { role, content }) = resumed.input.last() else {
        panic!("budget suffix missing");
    };
    assert_eq!(role, "system");
    let used = provider.estimate(resumed).unwrap().total;
    assert!(content.contains(&format!("used_bytes: {used:020}")));
    if let Some(after) = report.after {
        assert!(after.total > content.len() as i64);
    }
}

fn request(first: &str, last: &str, dry: bool, capsule: bool) -> compact::Exec {
    compact::Exec {
        first: Some(first.into()),
        last: Some(last.into()),
        from: None,
        to: None,
        summary: "completed earlier exchange".into(),
        absorb: Vec::new(),
        capsule: capsule.then(|| compact::Capsule {
            source: "test".into(),
            reason: "make room while a tool is active".into(),
            risk: "exact wording omitted".into(),
            queryability: None,
        }),
        dry,
    }
}

pub(super) async fn opened(store: &santi_core::Store, strand: &str) {
    let now = santi_core::now();
    pinned(
        store,
        strand,
        Seed {
            tag: "turn_active",
            text: "active input",
            effective: &fixed(crate::SCENARIO, "flash"),
        },
    )
    .await;
    store
        .ponder(Thinking {
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
                continuation: Some(Box::new(state("active reasoning", 2))),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    called(store, "call_active").await;
}

async fn called(store: &santi_core::Store, tag: &str) {
    store
        .calling(Call {
            tag,
            turn: "turn_active",
            tool: "shell",
            arguments: &json!({"command":"printf completed"}),
            created: &santi_core::now(),
        })
        .await
        .unwrap();
}

pub(super) async fn closed(store: &santi_core::Store, strand: &str) {
    called(store, "call_second").await;
    for (tag, call) in [
        ("reply_active", "call_active"),
        ("reply_second", "call_second"),
    ] {
        store
            .replying(Reply {
                tag,
                call,
                output: Some(&json!({"exit_code":0})),
                error: None,
                created: &santi_core::now(),
            })
            .await
            .unwrap();
    }
    let to = store.strand(strand).await.unwrap().unwrap().next - 1;
    store
        .complete("turn_active", to, &santi_core::now())
        .await
        .unwrap();
}

#[tokio::test]
async fn refused() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;

    let provider = Arc::new(Script::new(vec![tools(), finish("answer")]));
    let service = Service::open(crate::SCENARIO, config(&temp), provider)
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let messages = service.strand(&strand.id).await.unwrap().unwrap().messages;
    assert!(messages.len() >= 3, "the tool round must be recorded");

    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    let before = store.compacts(&strand.id).await.unwrap().len();

    let refused = service
        .exec(
            &strand.id,
            compact::Exec {
                first: Some(messages[0].message.id.clone()),
                last: Some(messages[1].message.id.clone()),
                from: None,
                to: None,
                summary: "cut the tool round".into(),
                absorb: Vec::new(),
                capsule: None,
                dry: false,
            },
        )
        .await
        .expect_err("an unsafe explicit range must be refused before commit");
    assert!(
        refused.contains("compact.anchor") || refused.contains("compact.exchange"),
        "the refusal must name the structural conflict: {refused}"
    );
    assert_eq!(
        store.compacts(&strand.id).await.unwrap().len(),
        before,
        "a refused explicit range leaves Compact unchanged"
    );
}
