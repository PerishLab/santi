use super::turn::{STRAND, choice, draft, driver, start};
use santi_estate::Opening;
use santi_model::receipt;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent() {
    let (_temp, _path, store) = start().await;
    let first = choice("provider_a", "model_a", Some("low"));
    let second = choice("provider_b", "model_b", Some("high"));
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut tasks = Vec::new();
    for (tag, effective) in [("turn_a", first.clone()), ("turn_b", second.clone())] {
        let store = store.clone();
        let ready = barrier.clone();
        tasks.push(tokio::spawn(async move {
            ready.wait().await;
            store.begin(draft(tag), &effective).await
        }));
    }
    let mut begun = None;
    let mut running = None;
    for task in tasks {
        match task.await.expect("task") {
            Ok(Opening::Started(opened)) => begun = Some(opened),
            Ok(Opening::Running(opened)) => running = Some(opened),
            Ok(other) => panic!("unexpected opening: {other:?}"),
            Err(error) => panic!("concurrent drain failed: {error}"),
        }
    }
    let begun = begun.expect("one started opening");
    let running = running.expect("one running opening");
    let snapshot = begun.turn.inference.clone().expect("frozen snapshot");
    assert!(
        snapshot == first || snapshot == second,
        "not a candidate: {snapshot:?}"
    );
    assert_eq!(running.id, begun.turn.id);
    assert_eq!(running.inference.as_ref(), Some(&snapshot));

    let turns = store.turns(STRAND).await.expect("turns");
    assert_eq!(
        turns.len(),
        1,
        "concurrent drain created extra turns: {turns:?}"
    );
    assert_eq!(turns[0].id, begun.turn.id);
    assert_eq!(turns[0].inference.as_ref(), Some(&snapshot));
    assert_eq!(
        store.inference(&begun.turn.id).await.expect("inference"),
        Some(snapshot.clone())
    );
    assert!(store.inboxes(STRAND).await.expect("inbox").is_empty());
    assert_eq!(store.messages(STRAND).await.expect("messages").len(), 1);
    let status = store
        .receipt("inbox_one")
        .await
        .expect("receipt")
        .expect("status");
    assert_eq!(status.state, receipt::State::Driving);
    assert_eq!(driver(&status), Some(begun.turn.id.as_str()));
}
