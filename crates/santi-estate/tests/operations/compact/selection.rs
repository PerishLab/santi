use santi_estate::{CompactDraft, Limits, MessageDraft, Store, StrandDraft};
use santi_model::{compact, message};

const CREATED: &str = "2026-10-22T00:00:00.000Z";
const LIMITS: Limits = Limits {
    slots: 8,
    settled: 1 << 20,
};

async fn strand(path: &std::path::Path, states: &[message::State]) -> Store {
    let store = super::super::support::bootstrap(path).await;
    store.seed("soul_test", CREATED).await.expect("seed");
    store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            created: CREATED,
        })
        .await
        .expect("strand");
    for (index, state) in states.iter().enumerate() {
        store
            .place(MessageDraft {
                tag: &format!("m{index}"),
                strand: "strand_test",
                actor: message::Role::Soul,
                actor_id: "soul_test",
                kind: message::Kind::Text,
                content: &message::Content::text("settled exchange"),
                state: state.clone(),
                request: false,
                created: CREATED,
            })
            .await
            .expect("message");
    }
    store
}

async fn occupy(store: &Store, tag: &str, first: &str, last: &str) {
    store
        .create_compact(CompactDraft {
            tag,
            strand: "strand_test",
            first,
            last,
            summary: "occupied",
            metadata: None,
            expected: None,
            created: CREATED,
        })
        .await
        .expect("occupy slot");
}

fn exec(first: Option<&str>, last: Option<&str>, absorb: &[&str]) -> compact::Exec {
    compact::Exec {
        first: first.map(Into::into),
        last: last.map(Into::into),
        from: None,
        to: None,
        summary: "settled exchange".into(),
        absorb: absorb.iter().map(|slot| slot.to_string()).collect(),
        capsule: None,
        dry: false,
    }
}

async fn accepted(store: &Store, request: &compact::Exec, limits: Limits) -> compact::Choice {
    let choice = store
        .selection("strand_test", request, limits)
        .await
        .expect("selection accepts");
    store
        .preview_compact(
            "compact_preview",
            "strand_test",
            &choice.range.first,
            &choice.range.last,
        )
        .await
        .expect("plan accepts");
    choice
}

fn forward(refusal: &str) -> Vec<compact::Exec> {
    let mut held = Vec::new();
    for piece in refusal.split("first: ").skip(1) {
        let (first, rest) = piece.split_once(", last: ").expect("ready last");
        let last = rest.split([' ', ';', ')']).next().expect("ready last id");
        held.push(exec(Some(first), Some(last), &[]));
    }
    for piece in refusal.split("absorb: [").skip(1) {
        let (slots, _) = piece.split_once(']').expect("ready absorb");
        held.push(exec(None, None, &slots.split(", ").collect::<Vec<_>>()));
    }
    held
}

async fn recovered(store: &Store, request: &compact::Exec, code: &str, limits: Limits) -> String {
    let refusal = store
        .selection("strand_test", request, limits)
        .await
        .expect_err("refused");
    assert!(refusal.starts_with(code), "{refusal}");
    let ways = forward(&refusal);
    assert!(!ways.is_empty(), "{refusal}");
    for way in &ways {
        accepted(store, way, limits).await;
    }
    refusal
}

fn range(first: &str, last: &str) -> compact::Range {
    compact::Range {
        first: first.into(),
        last: last.into(),
    }
}

#[tokio::test]
async fn aborted() {
    use message::State::{Aborted, Fixed};
    let temp = tempfile::tempdir().expect("temp");
    let store = strand(
        &temp.path().join("estate.sqlite"),
        &[Fixed, Fixed, Aborted, Fixed],
    )
    .await;
    let settled = accepted(&store, &exec(None, None, &[]), LIMITS).await;
    assert_eq!(settled.range, range("m0", "m1"));
    assert_eq!(settled.requested, None);
    let inward = accepted(&store, &exec(Some("m0"), Some("m2"), &[]), LIMITS).await;
    assert_eq!(inward.range, range("m0", "m1"));
    assert_eq!(inward.requested, Some(range("m0", "m2")));
    let refusal = store
        .selection("strand_test", &exec(Some("m2"), Some("m2"), &[]), LIMITS)
        .await
        .expect_err("no eligible boundary");
    assert!(refusal.starts_with("[compact.empty]"), "{refusal}");
}

#[tokio::test]
async fn hull() {
    use message::State::{Aborted, Fixed};
    let temp = tempfile::tempdir().expect("temp");
    let store = strand(
        &temp.path().join("estate.sqlite"),
        &[Fixed, Fixed, Fixed, Fixed, Aborted, Fixed, Fixed],
    )
    .await;
    occupy(&store, "compact_a", "m1", "m1").await;
    occupy(&store, "compact_b", "m5", "m5").await;
    let choice = accepted(
        &store,
        &exec(Some("m3"), Some("m3"), &["compact_a"]),
        LIMITS,
    )
    .await;
    assert_eq!(choice.range, range("m1", "m3"));
    let report = store
        .create_compact(CompactDraft {
            tag: "compact_hull",
            strand: "strand_test",
            first: &choice.range.first,
            last: &choice.range.last,
            summary: "hull",
            metadata: None,
            expected: None,
            created: CREATED,
        })
        .await
        .expect("compact hull");
    assert_eq!(report.absorbed, vec!["compact_a".to_string()]);
    let refusal = recovered(
        &store,
        &exec(Some("m2"), Some("m2"), &["compact_b"]),
        "[compact.overlap]",
        LIMITS,
    )
    .await;
    assert!(
        refusal.contains("compact_hull (covering m1 to m3)"),
        "{refusal}"
    );
    assert!(refusal.contains("first: m1, last: m5"), "{refusal}");
    assert!(refusal.contains("first: m5, last: m5"), "{refusal}");
}

#[tokio::test]
async fn refusals() {
    use message::State::{Aborted, Fixed};
    let temp = tempfile::tempdir().expect("temp");
    let store = strand(
        &temp.path().join("estate.sqlite"),
        &[Fixed, Fixed, Fixed, Aborted, Fixed, Fixed],
    )
    .await;
    occupy(&store, "compact_a", "m0", "m1").await;
    occupy(&store, "compact_b", "m2", "m2").await;
    recovered(
        &store,
        &exec(Some("m1"), Some("m4"), &[]),
        "[compact.overlap]",
        LIMITS,
    )
    .await;
    recovered(
        &store,
        &exec(Some("m4"), Some("m2"), &[]),
        "[compact.order]",
        LIMITS,
    )
    .await;
    recovered(
        &store,
        &exec(Some("m3"), Some("m3"), &["compact_b"]),
        "[compact.empty]",
        LIMITS,
    )
    .await;
    let full = Limits { slots: 2, ..LIMITS };
    recovered(&store, &exec(None, None, &[]), "[compact.slots]", full).await;
    for (request, code) in [
        (exec(Some("m9"), Some("m4"), &[]), "[compact.unknown]"),
        (exec(Some("m4"), None, &[]), "[compact.form]"),
        (exec(None, None, &["compact_z"]), "[compact.absorb]"),
    ] {
        let refusal = store
            .selection("strand_test", &request, LIMITS)
            .await
            .expect_err("refused");
        assert!(refusal.starts_with(code), "{refusal}");
    }
}
