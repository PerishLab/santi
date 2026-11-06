use super::*;
use santi_estate::{Acceptance, DrainDraft, InboxDraft, ReplayDraft, WebhookDraft};
use santi_model::receipt::reply::Binding;
use serde_json::json;

#[tokio::test]
async fn inputs() {
    let directory = tempfile::tempdir().unwrap();
    let store = Store::bootstrap(
        directory.path().join("db"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    store.seed("soul_test", FIRST).await.unwrap();
    store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .unwrap();
    store
        .subscribe(WebhookDraft {
            name: "test",
            adaptor: "test",
            soul: "soul_test",
            strategy: "single",
            credential: "TEST_SECRET",
            created: FIRST,
        })
        .await
        .unwrap();
    for inbox in ["first", "second"] {
        store
            .windowed(
                InboxDraft {
                    tag: inbox,
                    strand: "strand_test",
                    kind: message::Kind::Text,
                    content: &message::Content::text(inbox),
                    source: None,
                    created: FIRST,
                },
                Acceptance {
                    replay: ReplayDraft::Webhook {
                        subscription: "test",
                        delivery: inbox,
                        digest: inbox,
                    },
                    gate: 10,
                    binding: Some(&Binding {
                        adaptor: "test".into(),
                        context: json!({}),
                        delay: 3000,
                    }),
                },
            )
            .await
            .unwrap();
    }
    assert!(store.receptions("strand_test").await.unwrap().is_empty());
    store
        .drain_turn(DrainDraft {
            turn: "turn_test",
            strand: "strand_test",
            trigger: turn::Trigger::System,
            actor: "santi",
            created: LATER,
        })
        .await
        .unwrap();
    let input = provider_input(&store, "strand_test").await.unwrap();
    assert_eq!(input.len(), 2);
    for item in input {
        let Item::Message { content, .. } = item else {
            panic!("input message expected")
        };
        let mut lines = content.lines();
        let message = lines
            .next()
            .unwrap()
            .strip_prefix("[message ")
            .unwrap()
            .strip_suffix(']')
            .unwrap();
        let inbox = lines
            .next()
            .unwrap()
            .strip_prefix("[reply ")
            .unwrap()
            .strip_suffix(']')
            .unwrap();
        assert_eq!(lines.next(), Some(inbox));
        assert_eq!(
            store
                .message(message)
                .await
                .unwrap()
                .unwrap()
                .message
                .content
                .rendered(),
            inbox
        );
    }
    store.complete_turn("turn_test", 2, LATER).await.unwrap();
    store
        .fork(ForkDraft {
            tag: "strand_child",
            parent: "strand_test",
            at: 2,
            inference: None,
            created: LATER,
        })
        .await
        .unwrap();
    assert!(store.receptions("strand_child").await.unwrap().is_empty());
    for item in provider_input(&store, "strand_child").await.unwrap() {
        if let Item::Message { content, .. } = item {
            assert!(!content.contains("[reply "));
        }
    }
}
