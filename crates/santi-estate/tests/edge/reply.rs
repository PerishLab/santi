use super::*;
use santi_estate::{Acceptance, Intent, Outcome};
use santi_model::receipt::reply::{Binding, Owner, Resolution, State};
use serde_json::{Value, json};

mod world;
use world::World;

#[tokio::test]
async fn takeover() {
    let world = World::new().await;
    assert!(world.accept("inbox_test", "digest").await.unwrap().inserted);
    assert!(
        !world
            .accept("inbox_ignored", "digest")
            .await
            .unwrap()
            .inserted
    );
    assert!(world.accept("inbox_conflict", "different").await.is_err());
    assert!(world.store.window("inbox_ignored").await.unwrap().is_none());
    assert_eq!(world.store.replies().await.unwrap().len(), 1);
    world.queue(Owner::Runtime, "runtime:1").await;
    let first = world.store.dispatch("inbox_test").await.unwrap().unwrap();
    assert_eq!(first.target, None);
    assert_eq!(first.uuid.len(), 32);
    world.queue(Owner::Soul, "final").await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
    world.settle("runtime:1", "accepted").await;
    let formal = world.store.dispatch("inbox_test").await.unwrap().unwrap();
    assert_eq!(formal.target.as_deref(), Some("om_reply"));
    world.settle("final", "accepted").await;
    world.queue(Owner::Runtime, "runtime:2").await;
    world.queue(Owner::Soul, "final").await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
    assert!(
        world
            .store
            .queue(
                "inbox_test",
                Intent {
                    owner: Owner::Soul,
                    operation: "final",
                    body: &Value::Null
                }
            )
            .await
            .is_err()
    );
    assert!(
        world
            .store
            .queue(
                "inbox_test",
                Intent {
                    owner: Owner::Soul,
                    operation: "second",
                    body: &Value::Null
                }
            )
            .await
            .is_err()
    );
    assert_eq!(
        world
            .store
            .window("inbox_test")
            .await
            .unwrap()
            .unwrap()
            .window
            .attempts
            .len(),
        2
    );
    world.complete().await;
    world.store.end_strand("strand_test").await.unwrap();
    assert!(world.store.window("inbox_test").await.unwrap().is_none());
}

#[tokio::test]
async fn interrupted() {
    let world = World::new().await;
    world.accept("inbox_test", "digest").await.unwrap();
    world.queue(Owner::Runtime, "runtime:1").await;
    world.store.dispatch("inbox_test").await.unwrap().unwrap();
    world.queue(Owner::Soul, "final").await;
    let world = world.restart().await;
    let unknown = world
        .store
        .window("inbox_test")
        .await
        .unwrap()
        .unwrap()
        .window
        .attempts[0]
        .clone();
    assert_eq!(unknown.state, State::Unknown);
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
    world.complete().await;
    assert!(world.store.end_strand("strand_test").await.is_err());
    assert!(
        world
            .store
            .resolution(
                "inbox_test",
                &Resolution {
                    operation: "runtime:1".into(),
                    applied: true,
                    message: None,
                    evidence: "settled".into()
                }
            )
            .await
            .is_err()
    );
    world
        .store
        .resolution(
            "inbox_test",
            &Resolution {
                operation: "runtime:1".into(),
                applied: true,
                message: Some("om_observed".into()),
                evidence: "original request remotely settled; exact message observed".into(),
            },
        )
        .await
        .unwrap();
    let retained = world
        .store
        .window("inbox_test")
        .await
        .unwrap()
        .unwrap()
        .window
        .attempts[0]
        .clone();
    assert_eq!(retained.result, unknown.result);
    assert!(retained.resolution.is_some());
    let formal = world.store.dispatch("inbox_test").await.unwrap().unwrap();
    assert_eq!(formal.target.as_deref(), Some("om_observed"));
    world
        .store
        .delivered(
            "inbox_test",
            Outcome {
                operation: "final",
                result: &json!({"state":"accepted"}),
                message: Some("om_foreign"),
            },
        )
        .await
        .unwrap_err();
    world
        .store
        .delivered(
            "inbox_test",
            Outcome {
                operation: "final",
                result: &json!({"state":"rejected"}),
                message: None,
            },
        )
        .await
        .unwrap();
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
    world.store.end_strand("strand_test").await.unwrap();
}

#[tokio::test]
async fn unsent() {
    let world = World::new().await;
    world.accept("inbox_test", "digest").await.unwrap();
    world.queue(Owner::Runtime, "runtime:1").await;
    world.queue(Owner::Soul, "final").await;
    let dispatched = world.store.dispatch("inbox_test").await.unwrap().unwrap();
    assert_eq!(dispatched.owner, Owner::Soul);
    assert_eq!(dispatched.target, None);
    let status = world.store.window("inbox_test").await.unwrap().unwrap();
    assert_eq!(status.window.attempts[0].state, State::Cancelled);
    world.settle("final", "unsent").await;
    world.queue(Owner::Soul, "final").await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
}

#[tokio::test]
async fn stale() {
    let world = World::new().await;
    world.accept("inbox_test", "digest").await.unwrap();
    world.queue(Owner::Runtime, "runtime:1").await;
    world.complete().await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
    let status = world.store.window("inbox_test").await.unwrap().unwrap();
    assert_eq!(status.window.attempts[0].state, State::Cancelled);
    world.queue(Owner::Soul, "final").await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_some());
}

#[tokio::test]
async fn concurrent() {
    let world = World::new().await;
    world.accept("inbox_test", "digest").await.unwrap();
    world.queue(Owner::Runtime, "runtime:1").await;
    let (a, b) = tokio::join!(
        world.store.dispatch("inbox_test"),
        world.store.dispatch("inbox_test")
    );
    assert_ne!(a.unwrap().is_some(), b.unwrap().is_some());
    world.settle("runtime:1", "rejected").await;
    world.queue(Owner::Soul, "final").await;
    assert!(
        world
            .store
            .dispatch("inbox_test")
            .await
            .unwrap()
            .unwrap()
            .target
            .is_none()
    );
}

#[tokio::test]
async fn editing() {
    let world = World::new().await;
    world.accept("inbox_test", "digest").await.unwrap();
    world.queue(Owner::Runtime, "runtime:1").await;
    world.store.dispatch("inbox_test").await.unwrap().unwrap();
    world.settle("runtime:1", "accepted").await;
    world
        .store
        .advance_receipt(santi_estate::ReceiptDraft {
            inbox: "inbox_test",
            state: santi_model::receipt::State::Recovered,
            turn: None,
            incident: None,
            occurred: LATER,
        })
        .await
        .unwrap();
    world.queue(Owner::Runtime, "runtime:2").await;
    let edit = world.store.dispatch("inbox_test").await.unwrap().unwrap();
    assert_eq!(edit.target.as_deref(), Some("om_reply"));
    world.queue(Owner::Soul, "final").await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
    world.settle("runtime:2", "unknown").await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
    world
        .store
        .resolution(
            "inbox_test",
            &Resolution {
                operation: "runtime:2".into(),
                applied: false,
                message: None,
                evidence: "original edit request settled remotely without application".into(),
            },
        )
        .await
        .unwrap();
    let formal = world.store.dispatch("inbox_test").await.unwrap().unwrap();
    assert_eq!(formal.target.as_deref(), Some("om_reply"));
    world.settle("final", "accepted").await;
    world.queue(Owner::Runtime, "runtime:3").await;
    assert!(world.store.dispatch("inbox_test").await.unwrap().is_none());
}
