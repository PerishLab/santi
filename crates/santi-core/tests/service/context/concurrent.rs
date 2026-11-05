use super::layers::*;

#[tokio::test]

async fn strands() {
    let f = open().await;
    let second = f.service.seat(&f.strand.soul).await.unwrap().strand;
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
    declare(
        &f.store,
        &second.id,
        Selection {
            provider: "beta",
            model: "beta-model",
            effort: "high",
        },
    )
    .await;
    enqueue(&f.store, &f.strand.id, "alpha request").await;
    enqueue(&f.store, &second.id, "beta request").await;

    let (a, b) = tokio::join!(f.service.drive(&f.strand.id), f.service.drive(&second.id));
    assert!(a.is_ok(), "{a:?}");
    assert!(b.is_ok(), "{b:?}");
    until(&f.store, &f.strand.id, santi_core::turn::Status::Running).await;
    until(&f.store, &second.id, santi_core::turn::Status::Running).await;

    let alpha = observed(&f.layers, "stream", "alpha request").await;
    let beta = observed(&f.layers, "stream", "beta request").await;
    assert_eq!(
        (
            alpha.provider.as_str(),
            alpha.model.as_str(),
            alpha.effort.as_deref()
        ),
        ("alpha", "alpha-model", Some("low"))
    );
    assert_eq!(
        (
            beta.provider.as_str(),
            beta.model.as_str(),
            beta.effort.as_deref()
        ),
        ("beta", "beta-model", Some("high"))
    );
    assert_eq!(
        f.layers
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.kind == "stream")
            .count(),
        2
    );

    f.layers.gate.send(true).unwrap();
}
