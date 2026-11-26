use super::world::*;
#[tokio::test]
async fn evidence() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.unwrap();
    active(&store).await;
    queued(&store, "selected").await;
    let batch = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    assert!(store.absorb(&batch).await.unwrap());
    let id = batch.messages()[0].message.id.clone();
    store
        .prepared("turn_test", 2, std::slice::from_ref(&id), LATER)
        .await
        .unwrap();
    store.prepared("absent", 3, &[], LATER).await.unwrap();
    drop(store);
    let store = Store::open(&path).await.unwrap();
    let events = store.events("strand_test").await.unwrap();
    let prepared: Vec<_> = events.iter().filter(|e| e.action == "prepared").collect();
    assert_eq!(prepared.len(), 1);
    assert_eq!(prepared[0].message, id);
    assert_eq!(
        prepared[0].payload,
        serde_json::json!({"kind":"input_prepared", "turn":"turn_test", "round":2})
    );
    assert_eq!(
        store.message(&id).await.unwrap().unwrap().message.version,
        1
    );
}

fn legacy() -> keel::spec::Spec {
    use keel::atom::Kind as Atom;
    use keel::bond::Kind as Bond;
    use keel::spec::{Rule, Spec};
    Spec::build("MessageEvent")
        .sole("tag", Atom::Text)
        .field("action", Atom::Text)
        .rule(
            "action",
            Rule::new().values(&["delete", "fix", "insert", "patch", "remove"]),
        )
        .field("actor_type", Atom::Text)
        .rule("actor_type", Rule::new().values(&["soul", "system"]))
        .field("actor", Atom::Text)
        .field("base_version", Atom::Int)
        .rule("base_version", Rule::new().min(1))
        .field("payload", Atom::Text)
        .field("created", Atom::Text)
        .root("message", Bond::Many2one, "Message")
        .freeze()
        .seal()
}

#[tokio::test]
async fn upgrade() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("estate.sqlite");
    let event = legacy();
    let mut graph = keel::Graph::new();
    for spec in santi_estate::graph().nodes().values() {
        if spec.name() != "MessageEvent" {
            graph.add(spec.clone());
        }
    }
    graph.add(event);
    let core = keel::bootstrap(graph, keel::adapt::db::Sqlite::file(&path).await.unwrap())
        .unwrap()
        .seal(SUDO)
        .await
        .unwrap();
    drop(core);
    let store = Store::open(&path).await.unwrap();
    active(&store).await;
    queued(&store, "selected").await;
    let batch = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    assert!(store.absorb(&batch).await.unwrap());
    store
        .prepared(
            "turn_test",
            2,
            &[batch.messages()[0].message.id.clone()],
            LATER,
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .events("strand_test")
            .await
            .unwrap()
            .iter()
            .filter(|e| e.action == "prepared")
            .count(),
        1
    );
}

#[tokio::test]
async fn recovery() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("estate.sqlite");
    let store = Store::bootstrap(&path, SUDO).await.unwrap();
    active(&store).await;
    queued(&store, "consumed").await;
    let batch = store
        .intake("strand_test", "turn_test", LATER)
        .await
        .unwrap()
        .unwrap();
    assert!(store.absorb(&batch).await.unwrap());
    queued(&store, "pending").await;
    drop(store);
    let store = Store::open(&path).await.unwrap();
    assert_eq!(store.recover("test", LATER).await.unwrap(), 1);
    let consumed = store.receipt("consumed").await.unwrap().unwrap();
    assert_eq!(consumed.state, receipt::State::Failed);
    assert_eq!(
        consumed.transitions.last().unwrap().turn.as_deref(),
        Some("turn_test")
    );
    assert_eq!(
        store.receipt("pending").await.unwrap().unwrap().state,
        receipt::State::Accepted
    );
    let events = store.events("strand_test").await.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| e.payload["kind"] == "inbox_drain" && e.payload["inbox"] == "consumed")
            .count(),
        1
    );
    assert_eq!(store.recover("test", LATER).await.unwrap(), 0);
    assert_eq!(store.turns("strand_test").await.unwrap().len(), 1);
}
