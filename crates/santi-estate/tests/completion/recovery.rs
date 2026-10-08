use super::{FIRST, LATER, bootstrap};
use santi_estate::{DrainDraft, EffectDraft, InboxDraft, Opening, Store, StrandDraft};
use santi_model::{effect, message, receipt, turn};

fn draft(tag: &str) -> DrainDraft<'_> {
    DrainDraft {
        turn: tag,
        strand: "strand_retry",
        trigger: turn::Trigger::StrandSend,
        source: None,
        actor: "santi",
        created: LATER,
    }
}

async fn failed(path: &std::path::Path) -> Store {
    let store = bootstrap(path).await;
    store.seed("soul_test", FIRST).await.expect("seed");
    store
        .create_strand(StrandDraft {
            tag: "strand_retry",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            created: FIRST,
        })
        .await
        .expect("strand");
    store
        .accept_inbox(
            InboxDraft {
                tag: "inbox_retry",
                strand: "strand_retry",
                kind: message::Kind::Text,
                content: &message::Content::text("original request"),
                source: None,
                created: FIRST,
            },
            10,
        )
        .await
        .expect("accept");
    assert!(matches!(
        store.drain_turn(draft("turn_failed")).await.expect("open"),
        Opening::Started(_)
    ));
    store
        .fail_turn("turn_failed", "provider failed", LATER)
        .await
        .expect("fail");
    store
}

#[tokio::test]
async fn retains() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("db");
    let store = failed(&path).await;
    assert!(matches!(
        store
            .drain_turn(draft("turn_automatic"))
            .await
            .expect("automatic"),
        Opening::Idle
    ));
    drop(store);
    let store = Store::open(path).await.expect("reopen");
    let (a, b) = tokio::join!(
        store.redrive(draft("turn_a")),
        store.redrive(draft("turn_b"))
    );
    let (started, running) = match (a.expect("a"), b.expect("b")) {
        (Opening::Started(a), Opening::Running(b)) => (a, b),
        (Opening::Running(b), Opening::Started(a)) => (a, b),
        _ => panic!("exactly one retry starts"),
    };
    assert_eq!(started.turn.id, running.id);
    assert_eq!(started.turn.from, 1);
    assert!(started.drained.is_empty());
    assert_eq!(
        store
            .messages("strand_retry")
            .await
            .expect("messages")
            .len(),
        1
    );
    assert!(
        store
            .inboxes("strand_retry")
            .await
            .expect("queue")
            .is_empty()
    );
    let status = store
        .receipt("inbox_retry")
        .await
        .expect("receipt")
        .expect("held");
    assert_eq!(status.state, receipt::State::Driving);
    assert_eq!(status.accepted, FIRST);
    assert_eq!(status.transitions.len(), 4);
    assert_eq!(
        status.transitions[3].turn.as_deref(),
        Some(started.turn.id.as_str())
    );
    store
        .complete_turn(&started.turn.id, 1, LATER)
        .await
        .expect("complete");
    let status = store
        .receipt("inbox_retry")
        .await
        .expect("receipt")
        .expect("held");
    assert_eq!(status.state, receipt::State::Completed);
    assert_eq!(status.transitions.len(), 5);
    assert!(matches!(
        store.redrive(draft("turn_again")).await.expect("settled"),
        Opening::Idle
    ));
}

#[tokio::test]
async fn refuses() {
    for state in [
        effect::State::Unknown,
        effect::State::Dispatching,
        effect::State::Settled(effect::Outcome::Applied),
    ] {
        let temp = tempfile::tempdir().expect("temp");
        let store = failed(&temp.path().join("db")).await;
        store
            .prepare_effect(EffectDraft {
                tag: "effect_retry",
                turn: "turn_failed",
                call: None,
                kind: "shell",
                metadata: None,
                created: FIRST,
            })
            .await
            .expect("effect");
        store
            .dispatch_effect("effect_retry", LATER)
            .await
            .expect("dispatch");
        match state {
            effect::State::Unknown => {
                store
                    .unknown_effect("effect_retry", "unknown", LATER)
                    .await
                    .expect("unknown");
            }
            effect::State::Dispatching => {}
            effect::State::Settled(effect::Outcome::Applied) => {
                store
                    .unknown_effect("effect_retry", "unknown", LATER)
                    .await
                    .expect("unknown");
                store
                    .settle_effect(
                        "effect_retry",
                        effect::Outcome::Applied,
                        "operator confirmed execution",
                        LATER,
                    )
                    .await
                    .expect("applied");
            }
            _ => unreachable!(),
        }
        let opening = store.redrive(draft("turn_retry")).await.expect("refusal");
        let Opening::Refused(refusal) = opening else {
            panic!("unsafe effect must refuse replay");
        };
        assert_eq!(refusal.inbox, "inbox_retry");
        assert_eq!(refusal.effect, "effect_retry");
        assert_eq!(
            refusal.state,
            match state {
                effect::State::Unknown => "unknown",
                effect::State::Dispatching => "dispatching",
                effect::State::Settled(effect::Outcome::Applied) => "settled_applied",
                _ => unreachable!(),
            }
        );
        assert!(store.turn("turn_retry").await.expect("turn").is_none());
        let status = store
            .receipt("inbox_retry")
            .await
            .expect("receipt")
            .expect("held");
        assert_eq!(status.state, receipt::State::Failed);
        assert_eq!(status.transitions.len(), 3);
        assert_eq!(status.effects[0].state, state);
    }
}

#[tokio::test]
async fn unapplied() {
    let temp = tempfile::tempdir().expect("temp");
    let store = failed(&temp.path().join("db")).await;
    store
        .prepare_effect(EffectDraft {
            tag: "effect_retry",
            turn: "turn_failed",
            call: None,
            kind: "shell",
            metadata: None,
            created: FIRST,
        })
        .await
        .expect("effect");
    store
        .reconcile_effects("turn_failed", LATER)
        .await
        .expect("not dispatched");
    assert!(matches!(
        store
            .redrive(draft("turn_retry"))
            .await
            .expect("safe retry"),
        Opening::Started(_)
    ));
}
