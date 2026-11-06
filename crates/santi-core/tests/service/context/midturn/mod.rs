mod boundary;
mod failure;
mod pending;
mod world;
use super::layers::*;
use std::sync::atomic::Ordering;

#[tokio::test]
async fn pin() {
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
    until(&f.store, &f.strand.id, santi_core::turn::Status::Running).await;
    observed(&f.layers, "stream", "first").await;

    let before = f.service.audit(&f.strand.id).await.unwrap().unwrap();
    assert_eq!(before.budget.unwrap().bytes, 200_000);
    let estimate = f
        .layers
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.kind == "estimate" && c.provider == "alpha")
        .count();
    assert!(
        estimate > 0,
        "audit must estimate through the pinned provider"
    );
    assert!(
        !f.layers
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.kind == "estimate" && c.provider == "beta")
    );

    declare(
        &f.store,
        &f.strand.id,
        Selection {
            provider: "beta",
            model: "beta-model",
            effort: "high",
        },
    )
    .await;
    let after = f.service.audit(&f.strand.id).await.unwrap().unwrap();
    assert_eq!(after.budget.unwrap().bytes, 200_000);
    let active = f
        .service
        .running(&f.strand.soul, None)
        .await
        .unwrap()
        .expect("active");
    assert_eq!(active.active[0].budget.as_ref().unwrap().bytes, 200_000);

    f.layers.gate.send(true).unwrap();
    until(&f.store, &f.strand.id, santi_core::turn::Status::Completed).await;
    enqueue(&f.store, &f.strand.id, "second").await;
    f.service.drive(&f.strand.id).await.unwrap();
    let next = observed(&f.layers, "stream", "second").await;
    assert_eq!(
        (
            next.provider.as_str(),
            next.model.as_str(),
            next.effort.as_deref()
        ),
        ("beta", "beta-model", Some("high"))
    );
}

#[tokio::test]
async fn race() {
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
    f.layers.switch.store(true, Ordering::SeqCst);
    enqueue(&f.store, &f.strand.id, "first").await;
    f.service.drive(&f.strand.id).await.unwrap();
    until(&f.store, &f.strand.id, santi_core::turn::Status::Running).await;
    let first = observed(&f.layers, "stream", "first").await;
    assert_eq!(first.provider, "alpha");
    f.layers.gate.send(true).unwrap();
    until(&f.store, &f.strand.id, santi_core::turn::Status::Completed).await;
    enqueue(&f.store, &f.strand.id, "second").await;
    f.service.drive(&f.strand.id).await.unwrap();
    let next = observed(&f.layers, "stream", "second").await;
    assert_eq!(next.provider, "beta");
}
