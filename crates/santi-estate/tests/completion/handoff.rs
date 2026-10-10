use super::*;
use santi_estate::InboxDraft;

#[tokio::test]
async fn atomic() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("estate.sqlite");
    let store = bootstrap(&path).await;
    store.seed("soul_test", FIRST).await.unwrap();
    create_strand(&store, "strand_complete", None).await;
    create_turn(&store, "turn_complete", "strand_complete").await;
    let content = message::Content::text("kind: execution_pause\nturn: turn_complete");
    let source = santi_model::ingest::Source::new("execution_pause").with_ref("turn_complete");
    let draft = || CompletionDraft {
        turn: "turn_complete",
        reply: None,
        provider: "test",
        model: "test",
        response: None,
        occurred: LATER,
    };
    let inbox = InboxDraft {
        tag: "inbox_pause",
        strand: "strand_complete",
        kind: message::Kind::SantiSystem,
        content: &content,
        source: Some(&source),
        created: LATER,
    };
    let mut wrong = inbox.clone();
    wrong.strand = "missing_strand";
    assert!(store.handoff(draft(), wrong).await.is_err());
    assert_eq!(
        store.turn("turn_complete").await.unwrap().unwrap().status,
        turn::Status::Running
    );
    assert!(store.inboxes("strand_complete").await.unwrap().is_empty());
    create_strand(&store, "strand_other", None).await;
    let mut collision = inbox.clone();
    collision.strand = "strand_other";
    store.accept_inbox(collision, 500).await.unwrap();
    assert!(store.handoff(draft(), inbox.clone()).await.is_err());
    assert_eq!(
        store.turn("turn_complete").await.unwrap().unwrap().status,
        turn::Status::Running
    );
    assert!(store.inboxes("strand_complete").await.unwrap().is_empty());
    let mut inbox = inbox;
    inbox.tag = "inbox_resume";
    store.handoff(draft(), inbox.clone()).await.unwrap();
    assert_eq!(
        store.turn("turn_complete").await.unwrap().unwrap().status,
        turn::Status::Completed
    );
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert!(store.handoff(draft(), inbox).await.is_err());
    assert_eq!(store.inboxes("strand_complete").await.unwrap().len(), 1);
    let pending = store.receipt("inbox_resume").await.unwrap().unwrap();
    assert_eq!(pending.state, receipt::State::Accepted);
}

#[tokio::test]
async fn cancellation() {
    let temp = tempfile::tempdir().unwrap();
    let store = bootstrap(temp.path().join("estate.sqlite")).await;
    store.seed("soul_test", FIRST).await.unwrap();
    create_strand(&store, "strand_complete", None).await;
    create_turn(&store, "turn_complete", "strand_complete").await;
    store
        .request_stop("turn_complete", turn::Cause::Operator, LATER)
        .await
        .unwrap();
    let content = message::Content::text("paused");
    let result = store
        .handoff(
            CompletionDraft {
                turn: "turn_complete",
                reply: None,
                provider: "test",
                model: "test",
                response: None,
                occurred: LATER,
            },
            InboxDraft {
                tag: "inbox_pause",
                strand: "strand_complete",
                kind: message::Kind::SantiSystem,
                content: &content,
                source: None,
                created: LATER,
            },
        )
        .await;
    assert!(result.is_err());
    assert!(store.inboxes("strand_complete").await.unwrap().is_empty());
}
