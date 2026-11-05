use super::super::{NOW, SUDO};
use santi_estate::{Begun, DrainDraft, InboxDraft, Opening, Store, StrandDraft, Target, TurnDraft};
use santi_model::inference::{Effective, Origin, Patch, Source};
use santi_model::{message, receipt, turn};

const LATER: &str = "2026-07-28T00:01:00.000Z";
const SOUL: &str = "soul_test";
pub(super) const STRAND: &str = "strand_test";

pub(super) fn choice(provider: &str, model: &str, effort: Option<&str>) -> Effective {
    Effective {
        provider: provider.to_string(),
        model: model.to_string(),
        reasoning_effort: effort.map(str::to_string),
        source: Source {
            provider: Origin::Strand,
            model: Origin::Strand,
            reasoning_effort: effort.map(|_| Origin::Strand),
        },
    }
}

pub(super) fn draft(turn: &str) -> DrainDraft<'_> {
    DrainDraft {
        turn,
        strand: STRAND,
        trigger: turn::Trigger::System,
        actor: "santi",
        created: LATER,
    }
}

pub(super) async fn start() -> (tempfile::TempDir, std::path::PathBuf, Store) {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    store.seed(SOUL, NOW).await.expect("seed");
    store
        .create_strand(StrandDraft {
            tag: STRAND,
            soul: SOUL,
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: NOW,
        })
        .await
        .expect("strand");
    accept(&store, "inbox_one", "hello").await;
    (temp, path, store)
}

async fn accept(store: &Store, tag: &str, text: &str) {
    store
        .accept_inbox(
            InboxDraft {
                tag,
                strand: STRAND,
                kind: message::Kind::Text,
                content: &message::Content::text(text),
                source: None,
                created: NOW,
            },
            10,
        )
        .await
        .expect("inbox");
}

pub(super) fn driver(status: &receipt::Status) -> Option<&str> {
    status
        .transitions
        .last()
        .and_then(|transition| transition.turn.as_deref())
}

async fn drain(store: &Store, tag: &str, effective: &Effective) -> Begun {
    let opened = store.begin(draft(tag), effective).await.expect("drain");
    let Opening::Started(begun) = opened else {
        panic!("expected started turn, got {opened:?}");
    };
    begun
}

#[tokio::test]
async fn started() {
    let (_temp, _path, store) = start().await;
    let chosen = choice("provider_a", "model_a", Some("high"));
    let begun = drain(&store, "turn_one", &chosen).await;
    assert_eq!(begun.turn.id, "turn_one");
    assert_eq!(begun.turn.inference.as_ref(), Some(&chosen));

    let read = store
        .turn("turn_one")
        .await
        .expect("turn")
        .expect("present");
    assert_eq!(read.inference.as_ref(), Some(&chosen));

    let read = store
        .inference("turn_one")
        .await
        .expect("inference")
        .expect("recorded");
    assert_eq!(read, chosen);
    assert_eq!(read.provider, "provider_a");
    assert_eq!(read.model, "model_a");
    assert_eq!(read.reasoning_effort.as_deref(), Some("high"));

    assert!(store.inboxes(STRAND).await.expect("inbox").is_empty());
    let status = store
        .receipt("inbox_one")
        .await
        .expect("receipt")
        .expect("status");
    assert_eq!(status.state, receipt::State::Driving);
    assert_eq!(driver(&status), Some("turn_one"));
}

#[tokio::test]
async fn rollback() {
    let (_temp, _path, store) = start().await;
    let original = choice("provider_a", "model_a", Some("low"));
    let one = drain(&store, "turn_one", &original).await;
    store
        .complete_turn("turn_one", one.turn.from, LATER)
        .await
        .expect("complete");
    accept(&store, "inbox_retry", "retry").await;

    let replacement = choice("provider_b", "model_b", Some("high"));
    let failed = store.begin(draft("turn_one"), &replacement).await;
    assert!(
        failed.is_err(),
        "a completed turn tag was reused instead of failing: {failed:?}"
    );

    let inboxes = store.inboxes(STRAND).await.expect("inbox");
    assert_eq!(inboxes.len(), 1, "failed drain consumed input: {inboxes:?}");
    assert_eq!(inboxes[0].id, "inbox_retry");
    assert_eq!(store.messages(STRAND).await.expect("messages").len(), 1);
    assert_eq!(store.turns(STRAND).await.expect("turns").len(), 1);
    assert_eq!(
        store
            .turn("turn_one")
            .await
            .expect("turn")
            .expect("present")
            .inference
            .as_ref(),
        Some(&original)
    );
    assert_eq!(
        store.inference("turn_one").await.expect("inference"),
        Some(original)
    );

    let next = drain(&store, "turn_two", &replacement).await;
    assert_eq!(next.turn.inference.as_ref(), Some(&replacement));
    assert!(store.inboxes(STRAND).await.expect("inbox").is_empty());
}

#[tokio::test]
async fn legacy() {
    let (_temp, path, store) = start().await;
    let patch = Patch {
        provider: Some(Some("provider_b".to_string())),
        model: Some(Some("model_b".to_string())),
        reasoning_effort: Some(Some("high".to_string())),
    };
    store
        .declare(Target::Strand, STRAND, &patch, LATER)
        .await
        .expect("declare");

    let legacy = store
        .create_turn(TurnDraft {
            tag: "turn_legacy",
            strand: STRAND,
            trigger: turn::Trigger::StrandSend,
            from: 0,
            created: LATER,
        })
        .await
        .expect("legacy turn");
    assert!(legacy.inference.is_none());
    assert_eq!(
        store.inference("turn_legacy").await.expect("legacy"),
        None,
        "a turn without a snapshot must not adopt the current default"
    );

    drop(store);
    let store = Store::open(&path).await.expect("reopen");
    let legacy = store
        .turn("turn_legacy")
        .await
        .expect("legacy")
        .expect("present");
    assert!(legacy.inference.is_none());
    assert_eq!(
        store.inference("turn_legacy").await.expect("legacy"),
        None,
        "reopening must not recompute missing history"
    );
}

#[tokio::test]
async fn reopen() {
    let (_temp, path, store) = start().await;
    let old = choice("provider_a", "model_a", Some("low"));
    let one = drain(&store, "turn_one", &old).await;
    assert_eq!(one.turn.inference.as_ref(), Some(&old));
    store
        .complete_turn("turn_one", one.turn.from, LATER)
        .await
        .expect("complete");

    let patch = Patch {
        provider: Some(Some("provider_b".to_string())),
        model: Some(Some("model_b".to_string())),
        reasoning_effort: Some(Some("high".to_string())),
    };
    store
        .declare(Target::Strand, STRAND, &patch, LATER)
        .await
        .expect("declare");
    assert_eq!(
        store
            .turn("turn_one")
            .await
            .expect("turn")
            .expect("present")
            .inference
            .as_ref(),
        Some(&old)
    );

    accept(&store, "inbox_two", "next").await;
    let next = choice("provider_b", "model_b", Some("high"));
    let two = drain(&store, "turn_two", &next).await;
    assert_eq!(two.turn.inference.as_ref(), Some(&next));
    store
        .complete_turn("turn_two", two.turn.from, LATER)
        .await
        .expect("complete second");

    drop(store);
    let store = Store::open(&path).await.expect("reopen");
    for (tag, expected) in [("turn_one", Some(old)), ("turn_two", Some(next))] {
        let turn = store.turn(tag).await.expect("turn").expect("present");
        assert_eq!(turn.inference, expected, "restart snapshot {tag}");
        assert_eq!(
            store.inference(tag).await.expect("inference"),
            expected,
            "restart inference {tag}"
        );
    }
}
