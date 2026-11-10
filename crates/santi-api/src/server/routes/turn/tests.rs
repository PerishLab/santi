use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use santi_core::{inference, message, thinking, turn};
use santi_estate::{DrainDraft, InboxDraft, Opening, StrandDraft, ThinkingDraft};

use super::show;
use super::support::{ok, open, strand, turn};

#[tokio::test]
async fn turn_row() {
    let temp = tempfile::tempdir().expect("temp");
    let (store, service) = open(&temp).await;
    let strand = strand(&store).await;
    let first = turn(&store, &strand, "turn_first").await;
    let second = turn(&store, &strand, "turn_second").await;

    let Json(found) = ok(
        show(State(service.clone()), Path(first.id.clone())).await,
        "exact read",
    );
    assert_eq!(found.id, first.id);
    assert_eq!(found.strand, strand);
    assert_ne!(found.id, second.id);
    assert_eq!(found.status, turn::Status::Running);
    assert!(found.to.is_none());
    assert!(found.error.is_none());
    assert!(found.finished.is_none());
    assert!(found.finish.is_none());
    assert!(
        serde_json::to_value(&found)
            .expect("json")
            .get("inference")
            .is_none()
    );

    let missing = show(State(service), Path("turn_missing".to_string()))
        .await
        .expect_err("missing turn");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    assert_eq!(missing.code(), "resource.not_found");
    assert_eq!(missing.message(), "turn not found");
}

#[tokio::test]
async fn turn_ops() {
    let temp = tempfile::tempdir().expect("temp");
    let (store, service) = open(&temp).await;
    let strand = strand(&store).await;

    let running = turn(&store, &strand, "turn_running").await;

    let done = turn(&store, &strand, "turn_done").await;
    store
        .create_thinking(ThinkingDraft {
            tag: "thinking_done",
            turn: &done.id,
            response: Some("finished text"),
            created: &santi_core::now(),
        })
        .await
        .expect("thinking");
    store
        .chronicle(
            "thinking_done",
            &thinking::Round {
                status: Some("completed".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("round");
    let finished = santi_core::now();
    store
        .complete_turn(&done.id, 7, &finished)
        .await
        .expect("complete");

    let failed = turn(&store, &strand, "turn_failed").await;
    store
        .create_thinking(ThinkingDraft {
            tag: "thinking_failed",
            turn: &failed.id,
            response: Some("partial text"),
            created: &santi_core::now(),
        })
        .await
        .expect("thinking");
    store
        .chronicle(
            "thinking_failed",
            &thinking::Round {
                incomplete: Some("length".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect("round");
    store
        .fail_turn(&failed.id, "provider failed", &santi_core::now())
        .await
        .expect("fail");

    let Json(running) = ok(
        show(State(service.clone()), Path(running.id.clone())).await,
        "running",
    );
    assert_eq!(running.status, turn::Status::Running);
    assert!(running.to.is_none());
    assert!(running.error.is_none());
    assert!(running.finished.is_none());
    assert!(running.finish.is_none());

    let Json(done) = ok(
        show(State(service.clone()), Path(done.id.clone())).await,
        "completed",
    );
    assert_eq!(done.status, turn::Status::Completed);
    assert_eq!(done.to, Some(7));
    assert!(done.error.is_none());
    assert!(done.finished.is_some());
    let finish = done.finish.as_deref().expect("finish");
    assert_eq!(finish.span, "thinking_done");
    assert_eq!(finish.response.as_deref(), Some("finished text"));
    assert_eq!(finish.status.as_deref(), Some("completed"));
    assert!(finish.incomplete.is_none());

    let Json(failed) = ok(
        show(State(service.clone()), Path(failed.id.clone())).await,
        "failed",
    );
    assert_eq!(failed.status, turn::Status::Failed);
    assert!(failed.to.is_none());
    assert_eq!(failed.error.as_deref(), Some("provider failed"));
    assert!(failed.finished.is_some());
    let finish = failed.finish.as_deref().expect("finish");
    assert_eq!(finish.span, "thinking_failed");
    assert_eq!(finish.response.as_deref(), Some("partial text"));
    assert!(finish.status.is_none());
    assert_eq!(finish.incomplete.as_deref(), Some("length"));

    let fixture = store
        .create_strand(StrandDraft {
            tag: "ss_turn_inference",
            soul: santi_core::GENESIS,
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: &santi_core::now(),
        })
        .await
        .expect("inference strand")
        .id;
    let content = message::Content::text("inference fixture");
    store
        .accept_inbox(
            InboxDraft {
                tag: "inbox_inference",
                strand: &fixture,
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: &santi_core::now(),
            },
            500,
        )
        .await
        .expect("inbox");
    let effective = inference::Effective {
        provider: "test-provider".to_string(),
        model: "test-model".to_string(),
        reasoning_effort: None,
        source: inference::Source {
            provider: inference::Origin::Strand,
            model: inference::Origin::Strand,
            reasoning_effort: None,
        },
    };
    let Opening::Started(started) = store
        .begin(
            DrainDraft {
                turn: "turn_inference",
                strand: &fixture,
                trigger: turn::Trigger::StrandSend,
                actor: santi_core::SYSTEM,
                created: &santi_core::now(),
            },
            &effective,
        )
        .await
        .expect("begin")
    else {
        panic!("inference turn did not start");
    };
    store
        .complete_turn(&started.turn.id, started.turn.from, &santi_core::now())
        .await
        .expect("complete inference turn");

    let Json(inferred) = ok(
        show(State(service), Path(started.turn.id.clone())).await,
        "inference read",
    );
    assert_eq!(inferred.inference.as_ref(), Some(&effective));
}

#[tokio::test]
async fn turn_key() {
    let temp = tempfile::tempdir().expect("temp");
    let (store, service) = open(&temp).await;
    let strand = strand(&store).await;
    let held = turn(&store, &strand, "turn_held").await;
    store
        .complete_turn(&held.id, 3, &santi_core::now())
        .await
        .expect("complete");

    let before = (
        serde_json::to_value(
            store
                .turn(&held.id)
                .await
                .expect("before turn")
                .expect("held turn"),
        )
        .expect("before json"),
        serde_json::to_value(store.turns(&strand).await.expect("before turns"))
            .expect("before turns json"),
        store.running().await.expect("before running"),
    );

    let first = ok(
        show(State(service.clone()), Path(held.id.clone())).await,
        "first read",
    );
    let second = ok(
        show(State(service), Path(held.id.clone())).await,
        "second read",
    );
    assert_eq!(
        serde_json::to_value(first.0).expect("first json"),
        serde_json::to_value(second.0).expect("second json")
    );

    let after = (
        serde_json::to_value(
            store
                .turn(&held.id)
                .await
                .expect("after turn")
                .expect("held turn"),
        )
        .expect("after json"),
        serde_json::to_value(store.turns(&strand).await.expect("after turns"))
            .expect("after turns json"),
        store.running().await.expect("after running"),
    );

    assert_eq!(before, after);
}
