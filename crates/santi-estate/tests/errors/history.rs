use super::*;

async fn strand(store: &Store, id: &str) {
    store
        .create_strand(StrandDraft {
            tag: id,
            soul: "soul_default",
            label: None,
            parent: None,
            fork: None,
            created: FIRST,
        })
        .await
        .expect("strand");
}

#[tokio::test]
async fn pages() {
    let temp = tempfile::tempdir().expect("temp");
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .expect("open");
    store.seed("soul_default", FIRST).await.expect("soul");
    strand(&store, "history").await;
    strand(&store, "other").await;
    let scope = Scope::new("strand", "history");
    let key = catalog::INTERNAL.key("strand", "history");
    let mut expected = Vec::new();
    for index in 0..1027 {
        let time = if index < 512 { FIRST } else { LATER };
        let incident = store
            .raise(draft(&scope, "retained"), time)
            .await
            .expect("raise")
            .incident
            .expect("id");
        expected.push((time.to_string(), incident));
        if index != 1026 {
            store
                .resolve(&key, "test", serde_json::json!({}), RESOLVED)
                .await
                .expect("resolve");
        }
    }
    expected.sort_by(|a, b| b.cmp(a));
    let expected = expected.into_iter().map(|(_, id)| id).collect::<Vec<_>>();
    assert_eq!(
        store
            .incidents(&scope, usize::MAX)
            .await
            .expect("old list cap")
            .len(),
        1000
    );
    assert_eq!(
        store
            .history(&scope, 0, None)
            .await
            .expect("clamp")
            .expect("owner")
            .len(),
        1
    );
    let first = store
        .history(&scope, 237, None)
        .await
        .expect("first")
        .expect("owner");

    let mut seen = first
        .into_iter()
        .map(|incident| incident.id)
        .collect::<Vec<_>>();
    let mut newer = draft(&scope, "new arrival");
    newer.descriptor = catalog::NOT_FOUND;
    newer.key = newer.descriptor.key("strand", "history");
    let arrival = store
        .raise(newer, REOPENED)
        .await
        .expect("arrive between pages")
        .incident
        .expect("id");
    loop {
        let page = store
            .history(&scope, 237, seen.last().map(String::as_str))
            .await
            .expect("page")
            .expect("owner");
        assert!(page.len() <= 237);
        if page.is_empty() {
            break;
        }
        seen.extend(page.into_iter().map(|incident| incident.id));
    }
    assert_eq!(seen, expected);
    assert!(!seen.contains(&arrival));
    assert_eq!(
        store
            .history(&scope, 1, None)
            .await
            .expect("fresh")
            .expect("owner")[0]
            .id,
        arrival
    );
    let foreign = store
        .raise(draft(&Scope::new("strand", "other"), "private"), FIRST)
        .await
        .expect("foreign")
        .incident
        .expect("id");
    for cursor in [&foreign, &"missing".to_string(), &"".to_string()] {
        assert!(
            store
                .history(&scope, 5, Some(cursor))
                .await
                .expect_err("scope isolation")
                .contains("incident cursor")
        );
    }
    assert!(
        store
            .history(&Scope::new("strand", "missing"), 5, None)
            .await
            .expect("missing owner")
            .is_none()
    );
    assert!(
        store
            .incidents(&Scope::new("runtime", "default"), 5)
            .await
            .expect("compatibility")
            .is_empty()
    );
}

#[tokio::test]
async fn inspection() {
    let temp = tempfile::tempdir().expect("temp");
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.expect("open");
    store.seed("soul_default", FIRST).await.expect("soul");
    strand(&store, "history").await;
    let soul = store
        .raise(
            draft(&Scope::new("soul", "soul_default"), "soul report"),
            FIRST,
        )
        .await
        .expect("soul incident")
        .incident
        .expect("id");
    let owner = Scope::new("soul", "soul_default");
    assert_eq!(
        store
            .history(&owner, 5, None)
            .await
            .expect("soul page")
            .expect("owner")[0]
            .id,
        soul
    );
    assert!(
        store
            .history(&owner, 5, Some(&soul))
            .await
            .expect("soul cursor")
            .expect("owner")
            .is_empty()
    );
    let scope = Scope::new("strand", "history");
    let first = store
        .raise(draft(&scope, "first"), FIRST)
        .await
        .expect("open")
        .incident
        .expect("id");
    for message in ["middle", "latest"] {
        assert_eq!(
            store
                .raise(draft(&scope, message), LATER)
                .await
                .expect("repeat")
                .incident,
            Some(first.clone())
        );
    }
    let key = catalog::INTERNAL.key("strand", "history");
    store
        .resolve(
            &key,
            "operator",
            serde_json::json!({"fixed": true}),
            RESOLVED,
        )
        .await
        .expect("resolve");
    let reopened = store
        .raise(draft(&scope, "reopened"), REOPENED)
        .await
        .expect("reopen")
        .incident
        .expect("id");
    assert_ne!(first, reopened);
    let pending = store.pending_errors(20).await.expect("pending");
    let history = store
        .inspect(&first)
        .await
        .expect("inspect")
        .expect("retained");
    assert_eq!(history.incident.occurrences, 3);
    assert_eq!(history.incident.first.message, "first");
    assert_eq!(history.incident.latest.message, "latest");
    assert_eq!(history.transitions.len(), 2);
    assert_eq!(history.transitions[0].kind, Kind::Opened);
    assert_eq!(history.transitions[0].held.occurrences, 1);
    assert_eq!(history.transitions[0].held.latest.message, "first");
    assert_eq!(history.transitions[1].kind, Kind::Resolved);
    assert_eq!(history.transitions[1].held.occurrences, 3);
    assert_eq!(history.incident.scope, scope);
    assert_eq!(
        store
            .pending_errors(20)
            .await
            .expect("read does not ack")
            .len(),
        pending.len()
    );
    for event in &pending {
        store.deliver_error(&event.id, REOPENED).await.expect("ack");
    }
    drop(store);
    let store = Store::open(&path).await.expect("restart");
    assert!(
        store
            .pending_errors(20)
            .await
            .expect("delivered")
            .is_empty()
    );
    let history = store
        .inspect(&first)
        .await
        .expect("inspect delivered")
        .expect("retained");
    assert_eq!(history.transitions.len(), 2);
    assert_eq!(history.incident.status, Status::Resolved);
    assert_eq!(
        store
            .inspect(&reopened)
            .await
            .expect("new episode")
            .expect("active")
            .incident
            .status,
        Status::Active
    );
    assert_eq!(
        store
            .inspect(&soul)
            .await
            .expect("soul")
            .expect("retained")
            .incident
            .scope
            .kind,
        "soul"
    );
    assert!(store.inspect("missing").await.expect("missing").is_none());
    store.end_strand("history").await.expect("retire");
    for id in [&first, &reopened] {
        assert!(store.inspect(id).await.expect("retired").is_none());
    }
    assert!(
        store
            .inspect(&soul)
            .await
            .expect("unrelated soul")
            .is_some()
    );
}
