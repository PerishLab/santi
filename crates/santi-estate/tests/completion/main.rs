use santi_error::{Ruled, Scope};
use santi_estate::{Abort, Fault, Finish, Message, Store};
use santi_model::{budget, effect, message, receipt, turn};

mod handoff;
mod maintenance;
mod recovery;
mod support;
use support::*;

const LATER: &str = "2026-07-28T00:01:00.000Z";

#[tokio::test]
async fn ceremonies() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = bootstrap(&path).await;
    store.seed("soul_test", FIRST).await.expect("seed");
    support::strand(&store, "strand_complete", Some("worker/thread")).await;
    driven(&store, "complete").await;

    let scope = Scope::new("strand", "strand_complete");
    for error in [
        turn::Error::Provider.descriptor(),
        turn::Error::Runtime.descriptor(),
        budget::Error::Execution.descriptor(),
    ] {
        store
            .raise(incident(error, &scope, "prior failure"), FIRST)
            .await
            .expect("raise");
    }
    let content = message::Content::text("finished work");
    let reply = store
        .place(Message {
            tag: "message_reply",
            strand: "strand_complete",
            actor: message::Role::Soul,
            sender: "soul_test",
            kind: message::Kind::Text,
            content: &content,
            state: message::State::Fixed,
            request: false,
            created: LATER,
        })
        .await
        .expect("reply");
    let completed = store
        .finish(Finish {
            turn: "turn_complete",
            reply: Some(&reply.message.id),
            provider: "test-provider",
            model: "test-model",
            response: Some("response_test"),
            occurred: LATER,
        })
        .await
        .expect("finish");
    assert_eq!(completed.turn.status, turn::Status::Completed);
    assert_eq!(completed.turn.to, Some(1));
    let event = completed.event.expect("event");
    assert_eq!(event.text, "finished work");
    assert_eq!(event.label, "worker/thread");
    let strand = store
        .strand("strand_complete")
        .await
        .expect("strand")
        .expect("held");
    assert_eq!(strand.seen, 1);
    assert_eq!(
        strand.state,
        Some(serde_json::json!({
            "provider": "test-provider",
            "opaque": {"response_id": "response_test"},
            "schema_version": "santi-v1",
        }))
    );
    assert_eq!(
        store
            .receipt("inbox_complete")
            .await
            .expect("receipt")
            .expect("held")
            .state,
        receipt::State::Completed
    );
    for error in [
        turn::Error::Provider.descriptor(),
        turn::Error::Runtime.descriptor(),
        budget::Error::Execution.descriptor(),
    ] {
        assert!(
            store
                .incident(&error.key("strand", "strand_complete"))
                .await
                .expect("resolved")
                .is_none()
        );
    }
    let batch = store.outbox("turns", 0, "", 10).await.expect("outbox");
    assert_eq!(batch.events.len(), 1);
    assert_eq!(batch.events[0].id, event.id);

    support::strand(&store, "strand_foreign", None).await;
    let foreign = message::Content::text("foreign");
    store
        .place(Message {
            tag: "message_foreign",
            strand: "strand_foreign",
            actor: message::Role::Soul,
            sender: "soul_test",
            kind: message::Kind::Text,
            content: &foreign,
            state: message::State::Fixed,
            request: false,
            created: LATER,
        })
        .await
        .expect("foreign");
    support::turn(&store, "turn_cross", "strand_complete").await;
    assert!(
        store
            .finish(Finish {
                turn: "turn_cross",
                reply: Some("message_foreign"),
                provider: "test-provider",
                model: "test-model",
                response: None,
                occurred: LATER,
            })
            .await
            .is_err()
    );
    assert_eq!(
        store
            .turn("turn_cross")
            .await
            .expect("cross")
            .expect("turn")
            .status,
        turn::Status::Running
    );
    store
        .fail("turn_cross", "test cleanup", LATER)
        .await
        .expect("cleanup");

    support::turn(&store, "turn_stopped", "strand_complete").await;
    store
        .halt("turn_stopped", turn::Cause::Operator, FIRST)
        .await
        .expect("stop");
    assert!(
        store
            .finish(Finish {
                turn: "turn_stopped",
                reply: None,
                provider: "test-provider",
                model: "test-model",
                response: None,
                occurred: LATER,
            })
            .await
            .is_err()
    );
    assert!(
        store
            .strand("strand_complete")
            .await
            .expect("strand")
            .expect("held")
            .state
            .is_some()
    );
    store
        .abort(Abort {
            turn: "turn_stopped",
            cause: turn::Cause::Shutdown,
            actor: "santi",
            occurred: LATER,
        })
        .await
        .expect("interrupt");

    support::strand(&store, "strand_failed", None).await;
    driven(&store, "failed").await;
    store
        .dispatching("effect_failed", FIRST)
        .await
        .expect("dispatch");
    let scope = Scope::new("strand", "strand_failed");
    assert!(
        store
            .classify(Fault {
                turn: "turn_failed",
                detail: "wrongly scoped",
                incident: incident(
                    turn::Error::Provider.descriptor(),
                    &Scope::new("strand", "strand_complete"),
                    "provider failed",
                ),
                occurred: LATER,
            })
            .await
            .is_err()
    );
    assert_eq!(
        store
            .turn("turn_failed")
            .await
            .expect("failed turn")
            .expect("held")
            .status,
        turn::Status::Running
    );
    let failed = store
        .classify(Fault {
            turn: "turn_failed",
            detail: "provider unavailable",
            incident: incident(
                turn::Error::Provider.descriptor(),
                &scope,
                "provider failed",
            ),
            occurred: LATER,
        })
        .await
        .expect("classified failure");
    assert_eq!(failed.turn.status, turn::Status::Failed);
    assert_eq!(failed.turn.error.as_deref(), Some("provider unavailable"));
    assert!(failed.fault.incident.is_some());
    assert_eq!(
        store
            .effect("effect_failed")
            .await
            .expect("effect")
            .expect("held")
            .effect
            .state,
        effect::State::Unknown
    );
    let receipt = store
        .receipt("inbox_failed")
        .await
        .expect("receipt")
        .expect("held");
    assert_eq!(
        receipt
            .transitions
            .last()
            .and_then(|transition| transition.incident.as_deref()),
        failed.fault.incident.as_deref()
    );

    drop(store);
    let store = Store::open(path).await.expect("open again");
    assert_eq!(store.running().await.expect("running"), 0);
    assert_eq!(
        store
            .outbox("turns", 0, "", 10)
            .await
            .expect("outbox again")
            .events
            .len(),
        1
    );
}
