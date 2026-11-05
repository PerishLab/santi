use super::layers::*;

#[tokio::test]
async fn next() {
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
    enqueue(&f.store, &f.strand.id, "hold").await;
    f.service.drive(&f.strand.id).await.unwrap();
    until(&f.store, &f.strand.id, santi_core::turn::Status::Running).await;
    observed(&f.layers, "stream", "hold").await;

    declare(
        &f.store,
        &f.strand.id,
        Selection {
            provider: "missing",
            model: "missing-model",
            effort: "high",
        },
    )
    .await;
    let response = f.service.drive(&f.strand.id).await.expect("running drive");
    assert_eq!(response.state, santi_core::drive::State::Running);
    let audit = f.service.audit(&f.strand.id).await.expect("pinned audit");
    assert_eq!(audit.unwrap().budget.unwrap().bytes, 200_000);
    f.layers.gate.send(true).unwrap();
}

#[tokio::test]
async fn selection() {
    let f = open().await;
    declare(
        &f.store,
        &f.strand.id,
        Selection {
            provider: "missing",
            model: "missing-model",
            effort: "high",
        },
    )
    .await;
    enqueue(&f.store, &f.strand.id, "keep this input").await;
    let error = f
        .service
        .drive(&f.strand.id)
        .await
        .expect_err("invalid selection");
    assert!(
        format!("{error:?}").contains("provider missing"),
        "{error:?}"
    );
    assert!(f.store.latest(&f.strand.id).await.unwrap().is_none());
    let inboxes = f.store.inboxes(&f.strand.id).await.unwrap();
    assert!(
        inboxes
            .iter()
            .any(|inbox| { format!("{:?}", inbox.content).contains("keep this input") })
    );
    assert!(f.store.compacts(&f.strand.id).await.unwrap().is_empty());
    assert!(
        f.layers
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|call| call.kind != "stream")
    );
}
