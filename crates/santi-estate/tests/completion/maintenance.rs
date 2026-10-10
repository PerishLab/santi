use super::*;
use santi_estate::{Attempt, InboxDraft, Invocation, Limits, Maintenance};

fn invocation(index: usize, request: santi_model::compact::Exec) -> Invocation {
    Invocation {
        call: santi_model::tool::Call {
            id: format!("call_{index}"),
            turn: "turn_research".into(),
            tool: "compact".into(),
            arguments: serde_json::to_value(&request).unwrap(),
            created: LATER.into(),
        },
        result: format!("result_{index}"),
        attempt: Attempt::Compact(Box::new(request)),
        limit: Some(256),
    }
}

fn request(first: Option<&str>) -> santi_model::compact::Exec {
    santi_model::compact::Exec {
        first: first.map(str::to_string),
        last: first.map(str::to_string),
        from: None,
        to: None,
        summary: "work remains".into(),
        absorb: Vec::new(),
        capsule: None,
        dry: false,
    }
}

async fn opened(store: &Store) -> String {
    store.seed("soul_test", FIRST).await.unwrap();
    create_strand(store, "strand_research", None).await;
    let content = message::Content::text("work");
    store
        .accept_inbox(
            InboxDraft {
                tag: "inbox_work",
                strand: "strand_research",
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: FIRST,
            },
            500,
        )
        .await
        .unwrap();
    let opening = store
        .drain_turn(santi_estate::DrainDraft {
            turn: "turn_research",
            strand: "strand_research",
            trigger: turn::Trigger::System,
            source: None,
            actor: "santi",
            created: FIRST,
        })
        .await
        .unwrap();
    let santi_estate::Opening::Started(started) = opening else {
        panic!("not started")
    };
    store
        .place(MessageDraft {
            tag: "message_live",
            strand: "strand_research",
            actor: message::Role::Soul,
            actor_id: "soul_test",
            kind: message::Kind::Text,
            content: &content,
            state: message::State::Fixed,
            request: false,
            created: LATER,
        })
        .await
        .unwrap();
    started.drained[0].message.id.clone()
}

async fn maintain(
    store: &Store,
    calls: &[Invocation],
    inbox: &str,
) -> Result<santi_estate::Settlement, String> {
    let content = message::Content::text("kind: execution_pause");
    store
        .maintain(Maintenance {
            calls,
            limits: Limits {
                slots: 5,
                settled: 90000,
            },
            completion: CompletionDraft {
                turn: "turn_research",
                reply: Some("message_live"),
                provider: "test",
                model: "test",
                response: None,
                occurred: LATER,
            },
            inbox: InboxDraft {
                tag: inbox,
                strand: "strand_research",
                kind: message::Kind::SantiSystem,
                content: &content,
                source: None,
                created: LATER,
            },
        })
        .await
}

#[tokio::test]
async fn sequential() {
    for absorb in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("estate.sqlite");
        let store = bootstrap(&path).await;
        let first = opened(&store).await;
        if absorb {
            store
                .create_compact(santi_estate::CompactDraft {
                    tag: "compact_old",
                    strand: "strand_research",
                    first: &first,
                    last: &first,
                    summary: "old",
                    metadata: None,
                    expected: None,
                    created: FIRST,
                })
                .await
                .unwrap();
        }
        let mut second = request(None);
        if absorb {
            second.absorb.push("compact_old".into());
        }
        let calls = vec![
            invocation(
                0,
                if absorb {
                    request(Some(&first))
                } else {
                    request(None)
                },
            ),
            invocation(1, second),
        ];
        let result = maintain(&store, &calls, "inbox_pause").await.unwrap();
        assert!(result.completed);
        assert!(result.replies[0].error.is_none());
        let error = result.replies[1].error.as_deref().unwrap();
        assert!(error.contains(if absorb {
            "not an occupied slot"
        } else {
            "no settled range"
        }));
        assert!(
            result
                .replies
                .iter()
                .all(|reply| reply.output.as_ref().map_or_else(
                    || reply.error.as_ref().unwrap().len(),
                    |value| value.to_string().len()
                ) <= 256)
        );
        assert_eq!(store.compacts("strand_research").await.unwrap().len(), 1);
        let stop = store
            .request_stop("turn_research", turn::Cause::Operator, LATER)
            .await
            .unwrap()
            .unwrap();
        assert!(stop.cause.is_none());
        drop(store);
        let store = Store::open(&path).await.unwrap();
        assert_eq!(store.recover_turns("test.restart", LATER).await.unwrap(), 0);
        assert_eq!(
            store.turn("turn_research").await.unwrap().unwrap().status,
            turn::Status::Completed
        );
        assert_eq!(
            store.receipt("inbox_work").await.unwrap().unwrap().state,
            receipt::State::Completed
        );
        assert_eq!(store.inboxes("strand_research").await.unwrap().len(), 1);
        assert!(matches!(
            store
                .drain_turn(santi_estate::DrainDraft {
                    turn: "turn_next",
                    strand: "strand_research",
                    trigger: turn::Trigger::System,
                    source: None,
                    actor: "santi",
                    created: LATER
                })
                .await
                .unwrap(),
            santi_estate::Opening::Started(_)
        ));
    }
}

#[tokio::test]
async fn rollback() {
    for cause in [
        None,
        Some(turn::Cause::Operator),
        Some(turn::Cause::Shutdown),
    ] {
        let temp = tempfile::tempdir().unwrap();
        let store = bootstrap(temp.path().join("estate.sqlite")).await;
        let first = opened(&store).await;
        let mut calls = vec![
            invocation(0, request(Some(&first))),
            invocation(1, request(Some(&first))),
        ];
        if let Some(cause) = cause {
            store
                .request_stop("turn_research", cause, LATER)
                .await
                .unwrap();
        } else {
            calls[1].result = calls[0].result.clone();
        }
        assert!(maintain(&store, &calls, "inbox_pause").await.is_err());
        assert!(store.compacts("strand_research").await.unwrap().is_empty());
        assert!(store.calls("strand_research").await.unwrap().is_empty());
        assert!(store.results("strand_research").await.unwrap().is_empty());
        assert!(store.inboxes("strand_research").await.unwrap().is_empty());
        assert_eq!(
            store.turn("turn_research").await.unwrap().unwrap().status,
            turn::Status::Running
        );
    }
}

#[tokio::test]
async fn rejects() {
    let temp = tempfile::tempdir().unwrap();
    let store = bootstrap(temp.path().join("estate.sqlite")).await;
    opened(&store).await;
    let mut request = request(None);
    request.summary.clear();
    let mut rejected = invocation(1, request.clone());
    rejected.attempt = Attempt::Rejected("unsupported tool".into());
    let result = maintain(&store, &[invocation(0, request), rejected], "inbox_pause")
        .await
        .unwrap();
    assert!(!result.completed);
    assert_eq!(result.replies.len(), 2);
    assert!(result.replies.iter().all(|reply| reply.error.is_some()));
    assert!(store.compacts("strand_research").await.unwrap().is_empty());
    assert!(store.inboxes("strand_research").await.unwrap().is_empty());
    assert_eq!(
        store.turn("turn_research").await.unwrap().unwrap().status,
        turn::Status::Running
    );
}

#[tokio::test]
async fn stopped() {
    let temp = tempfile::tempdir().unwrap();
    let store = bootstrap(temp.path().join("estate.sqlite")).await;
    opened(&store).await;
    store
        .request_stop("turn_research", turn::Cause::Operator, LATER)
        .await
        .unwrap();
    let mut request = request(None);
    request.summary.clear();
    assert!(
        maintain(&store, &[invocation(0, request)], "inbox_pause")
            .await
            .is_err()
    );
    assert!(store.calls("strand_research").await.unwrap().is_empty());
    assert!(store.results("strand_research").await.unwrap().is_empty());
}
