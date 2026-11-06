use super::super::*;
use super::world::*;

#[tokio::test]
async fn provider() {
    failed(false).await;
}

#[tokio::test]
async fn stop() {
    failed(true).await;
}

async fn failed(stop: bool) {
    let f = open(Ending::Failure).await;
    let first = send(&f.service, &f.strand, "start").await;
    entered(&f.rounds, 0).await;
    let extra = send(&f.service, &f.strand, "unique_inbox_marker_369").await;
    let turn = f
        .service
        .receipt(&first.receipt.inbox)
        .await
        .unwrap()
        .unwrap()
        .transitions
        .iter()
        .find_map(|t| t.turn.clone())
        .unwrap();
    f.rounds.release[0].notify_one();
    entered(&f.rounds, 1).await;
    assert_eq!(markers(&f.rounds.requests.lock().unwrap()[1].2), 1);
    if stop {
        f.service.stop(&turn).await.unwrap();
    } else {
        f.rounds.release[1].notify_one();
    }
    super::super::layers::until(&f.store, &f.strand, santi_core::turn::Status::Failed).await;
    let store = Store::open(&f.config.database).await.unwrap();
    let receipt = store.receipt(&extra.receipt.inbox).await.unwrap().unwrap();
    assert_eq!(receipt.state, santi_core::receipt::State::Failed);
    assert_eq!(
        receipt
            .transitions
            .iter()
            .filter(|t| t.state == santi_core::receipt::State::Driving)
            .count(),
        1
    );
    assert_eq!(
        receipt.transitions.last().unwrap().turn.as_deref(),
        Some(turn.as_str())
    );
    let events = store.events(&f.strand).await.unwrap();
    assert_eq!(
        events
            .iter()
            .filter(
                |e| e.payload["kind"] == "inbox_drain" && e.payload["inbox"] == extra.receipt.inbox
            )
            .count(),
        1
    );
    assert_eq!(f.rounds.requests.lock().unwrap().len(), 2);
}
