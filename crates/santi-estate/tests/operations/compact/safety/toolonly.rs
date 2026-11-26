use super::*;

#[tokio::test]
async fn settled() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "start")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell"], 0, Some(1)))
        .await;
    writer.call("c0", "turn_test").await;
    writer.result("r0", "c0").await;
    writer
        .thought("s1", "turn_test", round(&["shell"], 0, Some(1)))
        .await;
    writer.call("c1", "turn_test").await;

    let choice = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("completed tool exchange can be compacted during the next exchange");
    assert_eq!(choice.range.first, "s0");
    assert_eq!(choice.range.last, "r0");
    let report = store
        .condense(Compact {
            tag: "compact_toolonly",
            strand: "strand_test",
            first: &choice.range.first,
            last: &choice.range.last,
            summary: "earlier shell finished",
            metadata: None,
            expected: None,
            created: CREATED,
        })
        .await
        .expect("commit");
    assert_eq!((report.from, report.to, report.collapsed), (2, 4, 3));
    let page = store
        .page("compact_toolonly", None, 0, 20)
        .await
        .expect("page")
        .expect("compact");
    assert_eq!(page.entries.len(), 3);
    assert_eq!(page.entries.last().expect("last").target, "r0");
    assert!(
        page.entries
            .iter()
            .all(|entry| entry.target != "s1" && entry.target != "c1")
    );
    writer.refuse(&named("s1", "c1"), "[compact.empty]").await;
    writer.refuse(&seated(2, 4), "[compact.unknown]").await;
    let child = store
        .fork(santi_estate::Fork {
            tag: "strand_child",
            parent: "strand_test",
            at: report.to,
            inference: None,
            created: LATER,
        })
        .await
        .expect("fork at the completed exchange");
    let inherited = store.compacts(&child.id).await.expect("child compacts");
    assert_eq!(inherited.len(), 1);
    let inherited = store
        .page(&inherited[0].id, None, 0, 20)
        .await
        .expect("child page")
        .expect("child compact");
    assert_eq!(inherited.entries.last().expect("child last").target, "r0");
    let rewrite = store
        .selection(
            "strand_test",
            &absorbing(None, None, &["compact_toolonly"]),
            LIMITS,
        )
        .await
        .expect("absorb entry-bound compact");
    assert_eq!(rewrite.range, choice.range);
    drop(store);
    let reopened = Store::open(temp.path().join("estate.sqlite"))
        .await
        .expect("reopen");
    let restored = reopened
        .compact("compact_toolonly")
        .await
        .expect("read compact")
        .expect("retained");
    assert_eq!(
        (restored.first.as_str(), restored.last.as_str()),
        ("s0", "r0")
    );
}

#[tokio::test]
async fn unanchored() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell", "shell"], 0, Some(2)))
        .await;
    writer.call("c0", "turn_test").await;
    writer.result("r0", "c0").await;
    writer.refuse(&default(), "[compact.settled]").await;
    writer.call("c1", "turn_test").await;
    writer.result("r1", "c1").await;
    let choice = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("complete tool-only exchange");
    assert_eq!(
        (choice.range.first.as_str(), choice.range.last.as_str()),
        ("s0", "r1")
    );
    let preview = store
        .preview("cmp_toolonly", "strand_test", "s0", "r0")
        .await
        .expect_err("cannot cut a completed multi-call exchange at an intermediate result");
    assert!(
        preview.contains("[compact.boundary]") || preview.contains("[compact.exchange]"),
        "{preview}"
    );
}

#[tokio::test]
async fn latest() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "input")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell"], 6, Some(1)))
        .await;
    writer
        .fixed("m1", message::Role::Soul, "soul_test", "latest")
        .await;
    writer.call("c0", "turn_test").await;
    writer.result("r0", "c0").await;
    let choice = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("range before the latest message");
    assert_eq!(
        (choice.range.first.as_str(), choice.range.last.as_str()),
        ("m0", "m0")
    );
}

#[tokio::test]
async fn selectors() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "input")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell"], 0, Some(1)))
        .await;
    writer.call("c0", "turn_test").await;
    writer.result("r0", "c0").await;
    store
        .place(Message {
            tag: "m1",
            strand: "strand_test",
            actor: message::Role::Soul,
            sender: "soul_test",
            kind: message::Kind::Text,
            content: &message::Content::text("aborted"),
            state: message::State::Aborted,
            request: false,
            created: LATER,
        })
        .await
        .expect("aborted Message");
    for request in [named("m0", "m1"), seated(1, 5)] {
        let choice = store
            .selection("strand_test", &request, LIMITS)
            .await
            .expect("legacy Message selectors normalize among Messages");
        assert_eq!(
            (choice.range.first.as_str(), choice.range.last.as_str()),
            ("m0", "m0")
        );
        assert!(choice.requested.is_some());
    }
    let choice = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("default can select complete exchange endpoints");
    assert_eq!(choice.range.last, "r0");
}
