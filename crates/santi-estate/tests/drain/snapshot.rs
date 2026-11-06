use super::world::*;
#[tokio::test]
async fn snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .unwrap();
    active(&store).await;
    queued(&store, "selected").await;
    notice(&store, 1).await;
    let stale = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    notice(&store, 2).await;
    assert!(!store.absorb(&stale).await.unwrap());
    assert!(store.messages("strand_test").await.unwrap().is_empty());
    assert_eq!(
        store.receipt("selected").await.unwrap().unwrap().state,
        receipt::State::Accepted
    );
    assert_eq!(
        store.receipt("notice_test").await.unwrap().unwrap().state,
        receipt::State::Accepted
    );
    let batch = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    queued(&store, "later").await;
    assert!(store.absorb(&batch).await.unwrap());
    assert!(!store.absorb(&batch).await.unwrap());
    let messages = store.messages("strand_test").await.unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].message.id, batch.messages()[0].message.id);
    assert_eq!(messages[1].message.id, batch.messages()[1].message.id);
    assert!(messages[1].text.contains("cause_1"));
    assert!(messages[1].text.contains("cause_2"));
    assert_eq!(
        store.receipt("later").await.unwrap().unwrap().state,
        receipt::State::Accepted
    );
    assert_eq!(store.inboxes("strand_test").await.unwrap().len(), 1);
    let events = store.events("strand_test").await.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.payload["kind"] == "inbox_drain")
            .count(),
        2
    );
    assert!(
        events
            .iter()
            .all(|e| e.payload["source"]["type"].is_null() || e.payload["source"]["type"] == "job")
    );
}

#[tokio::test]
async fn conflict() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.unwrap();
    active(&store).await;
    queued(&store, "selected").await;
    queued(&store, "second").await;
    let batch = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    store
        .place(santi_estate::MessageDraft {
            tag: &batch.messages()[1].message.id,
            strand: "strand_test",
            actor: message::Role::System,
            actor_id: "test",
            kind: message::Kind::Text,
            content: &message::Content::text("collision"),
            state: message::State::Fixed,
            request: false,
            created: FIRST,
        })
        .await
        .unwrap();
    assert!(!store.absorb(&batch).await.unwrap());
    assert_eq!(store.messages("strand_test").await.unwrap().len(), 1);
    assert_eq!(store.inboxes("strand_test").await.unwrap().len(), 2);
    for id in ["selected", "second"] {
        assert_eq!(
            store.receipt(id).await.unwrap().unwrap().state,
            receipt::State::Accepted
        );
    }
}

#[tokio::test]
async fn stopped() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .unwrap();
    active(&store).await;
    queued(&store, "selected").await;
    let batch = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    store
        .request_stop("turn_test", turn::Cause::Operator, LATER)
        .await
        .unwrap()
        .unwrap();
    assert!(!store.absorb(&batch).await.unwrap());
    assert!(
        store
            .intake("strand_test", "turn_test", LATER)
            .await
            .unwrap()
            .is_none()
    );
    assert!(store.messages("strand_test").await.unwrap().is_empty());
    assert!(store.events("strand_test").await.unwrap().is_empty());
    assert_eq!(store.inboxes("strand_test").await.unwrap().len(), 1);
    assert_eq!(
        store.receipt("selected").await.unwrap().unwrap().state,
        receipt::State::Accepted
    );
}

#[tokio::test]
async fn rollback() {
    let temp = tempfile::tempdir().unwrap();
    let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
        .await
        .unwrap();
    active(&store).await;
    queued(&store, "selected").await;
    queued(&store, "second").await;
    let batch = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    store
        .create_strand(StrandDraft {
            tag: "foreign",
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
        .place(santi_estate::MessageDraft {
            tag: &batch.messages()[1].message.id,
            strand: "foreign",
            actor: message::Role::System,
            actor_id: "test",
            kind: message::Kind::Text,
            content: &message::Content::text("collision"),
            state: message::State::Fixed,
            request: false,
            created: FIRST,
        })
        .await
        .unwrap();
    let before = store.strand("strand_test").await.unwrap().unwrap();
    assert!(store.absorb(&batch).await.is_err());
    assert_eq!(
        store.strand("strand_test").await.unwrap().unwrap().next,
        before.next
    );
    assert!(store.messages("strand_test").await.unwrap().is_empty());
    assert!(store.events("strand_test").await.unwrap().is_empty());
    assert_eq!(store.inboxes("strand_test").await.unwrap().len(), 2);
    for id in ["selected", "second"] {
        let receipt = store.receipt(id).await.unwrap().unwrap();
        assert_eq!(receipt.state, receipt::State::Accepted);
        assert_eq!(receipt.transitions.len(), 1);
        assert!(receipt.transitions[0].turn.is_none());
    }
    let foreign = store.messages("foreign").await.unwrap();
    assert_eq!(foreign.len(), 1);
    assert_eq!(foreign[0].message.id, batch.messages()[1].message.id);
    assert_eq!(foreign[0].text, "collision");
}
