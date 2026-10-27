use super::*;
use santi_estate::{DrainDraft, Opening};
use std::{sync::Arc, time::Duration};
use tokio::sync::Barrier;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn consumed() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    store.seed("soul_test", FIRST).await.expect("seed");
    for index in 0..20 {
        let strand = format!("strand_{index}");
        store
            .create_strand(StrandDraft {
                tag: &strand,
                soul: "soul_test",
                label: None,
                parent: None,
                fork: None,
                created: FIRST,
            })
            .await
            .expect("strand");
        let inbox = format!("inbox_{index}");
        let target = strand.clone();
        let consumer = store.clone();
        let barrier = Arc::new(Barrier::new(2));
        let ready = barrier.clone();
        let drain = tokio::spawn(consume(consumer, target, ready));
        let content = message::Content::text("accepted 工具🙂");
        let source = ingest::Source {
            kind: "test".into(),
            source: Some("external provenance".into()),
            metadata: Some(serde_json::json!({"sequence":index})),
        };
        barrier.wait().await;
        let accepted = store
            .accept_inbox(
                InboxDraft {
                    tag: &inbox,
                    strand: &strand,
                    kind: message::Kind::SantiSystem,
                    content: &content,
                    source: Some(&source),
                    created: FIRST,
                },
                1,
            )
            .await;
        let begun = tokio::time::timeout(Duration::from_secs(5), drain)
            .await
            .expect("bounded drain")
            .expect("consumer");
        let accepted = accepted.expect("a consumed committed input is still accepted");
        assert_eq!(accepted.id, inbox);
        assert_eq!(accepted.strand, strand);
        assert_eq!(accepted.content.rendered(), content.rendered());
        assert_eq!(accepted.created, FIRST);
        assert!(matches!(accepted.kind, message::Kind::SantiSystem));
        let held = accepted.source.expect("source");
        assert_eq!(held.kind, source.kind);
        assert_eq!(held.source, source.source);
        assert_eq!(held.metadata, source.metadata);
        assert!(accepted.coalesce_key.is_none());
        assert!(accepted.coalesce_revision.is_none());
        assert!(accepted.coalesce_causes.is_empty());
        assert_eq!(begun.drained[0].text, content.rendered());
        assert!(store.inbox(&inbox).await.expect("consumed queue").is_none());
        assert_eq!(
            store
                .receipt(&inbox)
                .await
                .expect("durable receipt")
                .expect("accepted")
                .state,
            receipt::State::Driving
        );
        let refused = format!("refused_{index}");
        assert!(
            store
                .accept_inbox(
                    InboxDraft {
                        tag: &refused,
                        strand: &strand,
                        kind: message::Kind::Text,
                        content: &content,
                        source: None,
                        created: FIRST,
                    },
                    0
                )
                .await
                .expect_err("capacity refusal")
                .contains("inbox is full")
        );
        assert!(
            store
                .receipt(&refused)
                .await
                .expect("no phantom receipt")
                .is_none()
        );
    }
}

async fn consume(store: Store, strand: String, ready: Arc<Barrier>) -> santi_estate::Begun {
    ready.wait().await;
    loop {
        let opened = store
            .drain_turn(DrainDraft {
                turn: &format!("turn_{strand}"),
                strand: &strand,
                trigger: turn::Trigger::StrandSend,
                actor: "santi",
                created: LATER,
            })
            .await
            .expect("drain");
        if let Opening::Started(begun) = opened {
            return begun;
        }
        tokio::task::yield_now().await;
    }
}
