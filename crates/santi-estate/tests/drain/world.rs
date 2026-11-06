pub(super) use santi_estate::{
    CallDraft, DrainDraft, EffectDraft, InboxDraft, NoticeDraft, Opening, Store, StrandDraft,
};
pub(super) use santi_model::{effect, ingest, message, receipt, turn};

pub(super) const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub(super) const FIRST: &str = "2026-07-28T00:00:00.000Z";
pub(super) const LATER: &str = "2026-07-28T00:01:00.000Z";

pub(super) async fn active(store: &Store) {
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
        .create_turn(santi_estate::TurnDraft {
            tag: "turn_test",
            strand: "strand_test",
            trigger: turn::Trigger::System,
            from: 0,
            created: FIRST,
        })
        .await
        .unwrap();
}

pub(super) async fn queued(store: &Store, tag: &str) {
    store
        .accept_inbox(
            InboxDraft {
                tag,
                strand: "strand_test",
                kind: message::Kind::Text,
                content: &message::Content::text(tag),
                source: None,
                created: LATER,
            },
            50,
        )
        .await
        .unwrap();
}

pub(super) async fn notice(store: &Store, revision: i64) {
    store
        .offer_notice(
            NoticeDraft {
                tag: "notice_test",
                strand: "strand_test",
                key: "notice",
                revision,
                digest: &format!("digest_{revision}"),
                content: &message::Content::text("same content"),
                source: &ingest::Source::new("job"),
                causes: &[format!("cause_{revision}")],
                created: LATER,
            },
            50,
        )
        .await
        .unwrap();
}
