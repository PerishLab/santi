use santi_estate::{
    CallDraft, MessageDraft, NoticeDraft, ReplyDraft, Store, StrandDraft, ThinkingDraft, TurnDraft,
    WakeLease, WakeOfferDraft,
};
use santi_model::{ingest, message, thinking, turn};

const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const NOW: &str = "2026-07-28T00:00:00.000Z";

#[tokio::test]
async fn ledger() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    let genesis = store.seed("soul_default", NOW).await.expect("seed");
    assert_eq!(genesis.id, "soul_default");
    assert_eq!(
        store.seed("soul_default", NOW).await.expect("reseed").id,
        genesis.id
    );

    let strand = store
        .create_strand(StrandDraft {
            tag: "ss_test",
            soul: &genesis.id,
            label: Some("primary"),
            parent: None,
            fork: None,
            created: NOW,
        })
        .await
        .expect("strand");
    assert_eq!(strand.next, 1);
    assert_eq!(strand.label.as_deref(), Some("primary"));

    let placed = store
        .place(MessageDraft {
            tag: "msg_test",
            strand: &strand.id,
            actor: message::Role::System,
            actor_id: "santi",
            kind: message::Kind::Text,
            content: &message::Content::text("hello"),
            state: message::State::Fixed,
            request: true,
            created: NOW,
        })
        .await
        .expect("place");
    assert_eq!(placed.relation.seq, 1);
    assert_eq!(placed.text, "hello");
    assert_eq!(
        store
            .strand(&strand.id)
            .await
            .expect("read")
            .expect("held")
            .next,
        2
    );
    assert_eq!(store.messages(&strand.id).await.expect("messages").len(), 1);

    let turn = store
        .create_turn(TurnDraft {
            tag: "turn_test",
            strand: &strand.id,
            trigger: turn::Trigger::StrandSend,
            source: Some("test"),
            from: 1,
            created: NOW,
        })
        .await
        .expect("turn");
    assert_eq!(turn.status, turn::Status::Running);
    assert_eq!(store.running().await.expect("running"), 1);

    let thinking = store
        .create_thinking(ThinkingDraft {
            tag: "thinking_test",
            turn: &turn.id,
            response: Some("response_test"),
            created: NOW,
        })
        .await
        .expect("thinking");
    store
        .update_thinking(&thinking.id, None, Some("summary"), NOW)
        .await
        .expect("summary");
    let thinking = store
        .complete_thinking(&thinking.id, thinking::Reason::Called, NOW)
        .await
        .expect("complete thinking")
        .expect("thinking");
    assert_eq!(thinking.state, thinking::State::Completed);

    let call = store
        .create_call(CallDraft {
            tag: "call_test",
            turn: &turn.id,
            tool: "shell",
            arguments: &serde_json::json!({"command": "true"}),
            created: NOW,
        })
        .await
        .expect("call");
    let reply = store
        .create_reply(ReplyDraft {
            tag: "result_test",
            call: &call.id,
            output: Some(&serde_json::json!({"ok": true})),
            error: None,
            created: NOW,
        })
        .await
        .expect("reply");
    assert_eq!(reply.output, Some(serde_json::json!({"ok": true})));
    assert!(
        store
            .create_reply(ReplyDraft {
                tag: "result_bad",
                call: &call.id,
                output: None,
                error: None,
                created: NOW,
            })
            .await
            .is_err()
    );

    let turn = store
        .complete_turn(&turn.id, 4, NOW)
        .await
        .expect("complete turn");
    assert_eq!(turn.status, turn::Status::Completed);
    assert_eq!(turn.to, Some(4));
    assert_eq!(store.running().await.expect("running"), 0);
    assert!(store.fail_turn(&turn.id, "late", NOW).await.is_err());

    let reopened = Store::open(temp.path().join("estate.sqlite"))
        .await
        .expect("reopen");
    assert_eq!(reopened.souls().await.expect("souls").len(), 1);
    assert_eq!(reopened.strands().await.expect("strands").len(), 1);
    assert_eq!(
        reopened.messages(&strand.id).await.expect("messages").len(),
        1
    );
}

#[tokio::test]
async fn lease() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("wake.sqlite"), SUDO)
        .await
        .expect("open");
    store.seed("soul_wake", NOW).await.expect("seed");
    let strand = store
        .create_strand(StrandDraft {
            tag: "strand_clock",
            soul: "soul_wake",
            label: Some("santi:clock"),
            parent: None,
            fork: None,
            created: NOW,
        })
        .await
        .expect("clock strand");
    assert!(store.wake("soul_wake").await.expect("wake").is_none());
    let lease = store
        .enable_wake("soul_wake", 1_000, 100, NOW)
        .await
        .expect("enable");
    assert_eq!((lease.generation, lease.remaining), (1, 3));
    assert_eq!(
        store
            .enable_wake("soul_wake", 9_000, 900, NOW)
            .await
            .expect("repeat enable"),
        lease
    );
    assert!(offer(&store, &strand.id, &lease, 105).await.is_some());
    let mut current = store.wake("soul_wake").await.expect("read").expect("lease");
    assert_eq!((current.remaining, current.next_millis), (2, Some(1_105)));
    assert_eq!(current.last_wake_millis, Some(105));
    assert!(offer(&store, &strand.id, &lease, 105).await.is_none());
    current = store
        .renew_wake("soul_wake", 1, 999, NOW)
        .await
        .expect("renew");
    assert_eq!((current.remaining, current.next_millis), (3, Some(1_105)));
    let silent = store
        .silence_wake("soul_wake", 1, NOW)
        .await
        .expect("silence");
    assert_eq!((silent.state.as_str(), silent.generation), ("silent", 2));
    assert_eq!(
        store
            .silence_wake("soul_wake", 1, NOW)
            .await
            .expect("replay silence"),
        silent
    );
    assert!(store.renew_wake("soul_wake", 1, 999, NOW).await.is_err());
    let revoked = store
        .disable_wake("soul_wake", NOW)
        .await
        .expect("disable")
        .expect("lease");
    assert_eq!((revoked.state.as_str(), revoked.generation), ("revoked", 3));
    assert!(store.renew_wake("soul_wake", 3, 999, NOW).await.is_err());

    current = store
        .enable_wake("soul_wake", 1_000, 200, NOW)
        .await
        .expect("invite again");
    for expected in [2, 1, 0] {
        let due = current.next_millis.expect("next");
        assert!(offer(&store, &strand.id, &current, due + 5).await.is_some());
        current = store.wake("soul_wake").await.expect("read").expect("lease");
        assert_eq!(current.remaining, expected);
    }
    assert_eq!(current.state, "expired");
    let revived = store
        .renew_wake("soul_wake", current.generation, 9_999, NOW)
        .await
        .expect("renew final round");
    assert_eq!((revived.state.as_str(), revived.remaining), ("active", 3));
}

async fn offer(
    store: &Store,
    strand: &str,
    lease: &WakeLease,
    now: i64,
) -> Option<santi_estate::Offer> {
    let due = lease.next_millis.expect("scheduled lease");
    let tag = format!("inbox_{due}");
    let content = message::Content::text(format!("current_time: {due}"));
    let source = ingest::Source::new("clock").with_ref(lease.soul.clone());
    store
        .offer_wake(WakeOfferDraft {
            soul: &lease.soul,
            generation: lease.generation,
            due_millis: due,
            now_millis: now,
            occurred: NOW,
            notice: NoticeDraft {
                tag: &tag,
                strand,
                key: "clock/wake/soul_wake",
                revision: due,
                digest: "wake-digest",
                content: &content,
                source: &source,
                causes: &[],
                created: NOW,
            },
            gate: 10,
        })
        .await
        .expect("offer")
}
