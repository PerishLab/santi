use super::*;

#[tokio::test]
async fn pending() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "start")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell", "shell"], 6, Some(2)))
        .await;
    writer
        .fixed("m1", message::Role::Soul, "soul_test", "assistant")
        .await;
    writer.call("c0", "turn_test").await;
    writer.result("r0", "c0").await;
    writer
        .fixed("m2", message::Role::System, "santi", "after one call")
        .await;
    writer
        .fixed("m3", message::Role::Soul, "soul_test", "latest")
        .await;

    writer.refuse(&seated(6, 6), "[compact.exchange]").await;
    writer
        .refuse(&named("m2", "m2"), "[compact.exchange]")
        .await;
    let fallback = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("safe initial fallback");
    assert_eq!(
        (fallback.range.first.as_str(), fallback.range.last.as_str()),
        ("m0", "m0")
    );
}

#[tokio::test]
async fn bounded() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "start")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell"], 6, Some(1)))
        .await;
    writer
        .fixed("m1", message::Role::Soul, "soul_test", "assistant")
        .await;
    writer.call("c0", "turn_test").await;
    writer.thought("s1", "turn_test", round(&[], 0, None)).await;
    writer
        .fixed("m2", message::Role::System, "santi", "fresh")
        .await;
    writer
        .fixed("m3", message::Role::System, "santi", "settled")
        .await;
    writer
        .fixed("m4", message::Role::Soul, "soul_test", "latest")
        .await;

    let earliest = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("earliest safe range");
    assert_eq!(
        (earliest.range.first.as_str(), earliest.range.last.as_str()),
        ("m0", "m0")
    );
    writer
        .refuse(&named("m1", "m3"), "[compact.exchange]")
        .await;
    writer
        .refuse(&named("m0", "m3"), "[compact.exchange]")
        .await;
    store
        .selection("strand_test", &named("m2", "m3"), LIMITS)
        .await
        .expect("explicit later range");
}

#[tokio::test]
async fn relations() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("pre", message::Role::System, "santi", "start")
        .await;
    writer.turn("turn_test").await;
    writer.thought("s0", "turn_test", round(&[], 7, None)).await;
    writer
        .fixed("cut", message::Role::System, "santi", "between")
        .await;
    writer
        .fixed("text", message::Role::Soul, "soul_test", "assistant")
        .await;
    writer
        .fixed("live", message::Role::Soul, "soul_test", "latest")
        .await;

    writer
        .refuse(&named("pre", "cut"), "[compact.anchor]")
        .await;
    let whole = store
        .selection("strand_test", &named("pre", "text"), LIMITS)
        .await
        .expect("anchor and text together");
    assert_eq!(
        (whole.range.first.as_str(), whole.range.last.as_str()),
        ("pre", "text")
    );

    let legacy = opened(&temp.path().join("legacy.sqlite")).await;
    let old = Writer(&legacy);
    old.fixed("m0", message::Role::System, "santi", "start")
        .await;
    old.turn("turn_test").await;
    archaic(&legacy).await;
    old.fixed("m1", message::Role::Soul, "soul_test", "assistant")
        .await;
    old.call("c0", "turn_test").await;
    old.fixed("m2", message::Role::System, "santi", "mid").await;
    old.result("r0", "c0").await;
    old.fixed("m3", message::Role::System, "santi", "after")
        .await;
    old.fixed("m4", message::Role::Soul, "soul_test", "latest")
        .await;

    let textonly = legacy
        .selection("strand_test", &named("m0", "m1"), LIMITS)
        .await
        .expect("legacy text alone");
    assert_eq!(
        (textonly.range.first.as_str(), textonly.range.last.as_str()),
        ("m0", "m1")
    );
    old.refuse(&named("m1", "m2"), "[compact.exchange]").await;
    let enclosed = legacy
        .selection("strand_test", &named("m0", "m3"), LIMITS)
        .await
        .expect("whole legacy exchange");
    assert_eq!(
        (enclosed.range.first.as_str(), enclosed.range.last.as_str()),
        ("m0", "m3")
    );
}

async fn archaic(store: &Store) {
    store
        .create_thinking(ThinkingDraft {
            tag: "s0",
            turn: "turn_test",
            response: None,
            created: CREATED,
        })
        .await
        .expect("legacy thinking");
}

#[tokio::test]
async fn hull() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "start")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell"], 6, Some(1)))
        .await;
    writer
        .fixed("m1", message::Role::Soul, "soul_test", "assistant")
        .await;
    writer
        .fixed("m2", message::Role::System, "santi", "mid")
        .await;
    writer.call("c0", "turn_test").await;
    writer.result("r0", "c0").await;
    writer
        .fixed("m3", message::Role::System, "santi", "after")
        .await;
    writer
        .fixed("m4", message::Role::Soul, "soul_test", "latest")
        .await;
    store
        .create_compact(CompactDraft {
            tag: "compact_a",
            strand: "strand_test",
            first: "m0",
            last: "m0",
            summary: "occupied",
            metadata: None,
            expected: None,
            created: CREATED,
        })
        .await
        .expect("occupy");

    writer
        .refuse(
            &absorbing(Some("m1"), Some("m2"), &["compact_a"]),
            "[compact.exchange]",
        )
        .await;
    let alone = store
        .selection(
            "strand_test",
            &absorbing(None, None, &["compact_a"]),
            LIMITS,
        )
        .await
        .expect("absorb-only");
    assert_eq!(
        (alone.range.first.as_str(), alone.range.last.as_str()),
        ("m0", "m0")
    );
    let hull = store
        .selection(
            "strand_test",
            &absorbing(Some("m1"), Some("m3"), &["compact_a"]),
            LIMITS,
        )
        .await
        .expect("safe hull");
    assert_eq!(
        (hull.range.first.as_str(), hull.range.last.as_str()),
        ("m0", "m3")
    );
    let inward = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("default skips the unsafe left candidate");
    assert_eq!(inward.range.first, "m3");
    assert_eq!(inward.range.last, "m3");

    let bare = opened(&temp.path().join("bare.sqlite")).await;
    let other = Writer(&bare);
    other.turn("turn_test").await;
    other
        .thought("s0", "turn_test", round(&["shell"], 7, Some(1)))
        .await;
    other
        .fixed("n0", message::Role::Soul, "soul_test", "assistant")
        .await;
    other.call("c0", "turn_test").await;
    other.result("r0", "c0").await;
    other
        .fixed("n1", message::Role::Soul, "soul_test", "latest")
        .await;
    let error = other.refuse(&default(), "[compact.settled]").await;
    assert!(error.contains("name first/last or absorb"), "{error}");
}

#[tokio::test]
async fn matching() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "start")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell", "shell"], 6, Some(2)))
        .await;
    writer
        .fixed("m1", message::Role::Soul, "soul_test", "assistant")
        .await;
    writer.call("c0", "turn_test").await;
    writer.call("c1", "turn_test").await;
    writer.result("r0", "c0").await;
    writer
        .fixed("m2", message::Role::System, "santi", "after")
        .await;
    writer
        .fixed("m3", message::Role::Soul, "soul_test", "latest")
        .await;

    writer
        .refuse(&named("m0", "m2"), "[compact.exchange]")
        .await;

    writer.result("r1", "c1").await;
    writer
        .fixed("m4", message::Role::Soul, "soul_test", "last")
        .await;
    store
        .selection("strand_test", &named("m0", "m4"), LIMITS)
        .await
        .expect("completed range");
}
