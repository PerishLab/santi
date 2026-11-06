use super::*;
use santi_core::turn::{Status, activity::Phase};
use std::time::Duration;

#[tokio::test]
async fn lifecycle() {
    let f = open().await;
    declare(
        &f.store,
        &f.strand.id,
        Selection {
            provider: "alpha",
            model: "alpha-model",
            effort: "low",
        },
    )
    .await;
    enqueue(&f.store, &f.strand.id, "first").await;
    f.service.drive(&f.strand.id).await.unwrap();
    observed(&f.layers, "stream", "first").await;
    let mut receiver = f.service.observe(&f.strand.id).await.unwrap();
    let initial = receiver.borrow_and_update().clone();
    let current = initial.current.as_ref().unwrap();
    assert_eq!(current.phase, Phase::Requesting);
    assert_eq!(current.round, 1);
    let turn = current.turn.clone();
    enqueue(&f.store, &f.strand.id, "later input").await;
    assert_eq!(*receiver.borrow(), initial);
    f.layers.gate.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            receiver.changed().await.unwrap();
            if receiver.borrow_and_update().current.is_none() {
                break;
            }
        }
    })
    .await
    .unwrap();
    assert!(receiver.borrow().revision > initial.revision);
    let status = f.store.turn(&turn).await.unwrap().unwrap().status;
    assert_eq!(status, Status::Completed);
    assert!(f.service.observe("missing").await.is_err());
}
