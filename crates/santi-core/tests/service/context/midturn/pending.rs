use super::world::*;

#[tokio::test]
async fn oversized() {
    let f = open(Ending::Tools).await;
    let first = send(&f.service, &f.strand, "start").await;
    entered(&f.rounds, 0).await;
    let text = format!("unique_inbox_marker_369{}", "x".repeat(250_000));
    let extra = send(&f.service, &f.strand, &text).await;
    f.rounds.release[0].notify_one();
    entered(&f.rounds, 1).await;
    assert_eq!(markers(&f.rounds.requests.lock().unwrap()[1].2), 0);
    let receipt = f
        .service
        .receipt(&extra.receipt.inbox)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.state, santi_core::receipt::State::Accepted);
    assert!(receipt.transitions.iter().all(|t| t.turn.is_none()));
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
    f.service.stop(&turn).await.unwrap();
    super::super::layers::until(&f.store, &f.strand, santi_core::turn::Status::Failed).await;
}

#[tokio::test]
async fn terminal() {
    let f = open(Ending::Terminal).await;
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
    let receipt = f
        .service
        .receipt(&extra.receipt.inbox)
        .await
        .unwrap()
        .unwrap();
    let next = receipt
        .transitions
        .iter()
        .find_map(|t| t.turn.clone())
        .unwrap();
    assert_ne!(next, turn);
    assert_eq!(
        f.store.turn(&turn).await.unwrap().unwrap().status,
        santi_core::turn::Status::Completed
    );
    assert_eq!(markers(&f.rounds.requests.lock().unwrap()[1].2), 1);
    let events = f.store.events(&f.strand).await.unwrap();
    assert!(
        !events
            .iter()
            .any(|e| e.action == "prepared" && e.payload["turn"] == turn)
    );
    f.rounds.release[1].notify_one();
    super::super::layers::until(&f.store, &f.strand, santi_core::turn::Status::Completed).await;
}
