use super::*;

#[tokio::test]
async fn identity() {
    let mut harness = PromptHarness::open().await;
    let other = harness
        .service
        .awaken(santi_core::soul::Draft { memory: None })
        .await
        .unwrap();
    let identity = santi_core::soul::Identity {
        name: "Adam Santi".into(),
        channels: [("github".into(), "adam-example".into())].into(),
    };
    harness.service = harness
        .service
        .clone()
        .identified(
            [
                (santi_core::GENESIS.into(), identity.clone()),
                (
                    other.id.clone(),
                    santi_core::soul::Identity {
                        name: "Other Individual".into(),
                        channels: Default::default(),
                    },
                ),
            ]
            .into(),
        )
        .await
        .unwrap();
    let second = harness
        .service
        .seat(santi_core::GENESIS)
        .await
        .unwrap()
        .strand;
    for strand in [&harness.strand, &second.id] {
        let text = harness.system_prompt_for(strand).await.text;
        assert!(text.contains("name: \"Adam Santi\""));
        assert!(!text.contains("adam-example"));
        assert!(!text.contains("Other Individual"));
    }
    harness.write_soul("learned experience");
    let text = harness.system_prompt().await.text;
    assert!(text.contains("name: \"Adam Santi\""));
    assert!(text.contains("learned experience"));
    let other = harness.service.seat(&other.id).await.unwrap().strand;
    let text = harness.system_prompt_for(&other.id).await.text;
    assert!(text.contains("name: \"Other Individual\""));
    assert!(!text.contains("Adam Santi"));
    assert_eq!(
        harness
            .service
            .soul(santi_core::GENESIS)
            .await
            .unwrap()
            .unwrap()
            .identity,
        Some(identity)
    );
    assert!(
        harness
            .service
            .souls()
            .await
            .unwrap()
            .iter()
            .all(|soul| soul.identity.is_some())
    );
}

#[tokio::test]
async fn invalid() {
    let harness = PromptHarness::open().await;
    let identity = santi_core::soul::Identity {
        name: "Adam Santi".into(),
        channels: Default::default(),
    };
    assert!(
        harness
            .service
            .clone()
            .identified([("missing".into(), identity.clone())].into())
            .await
            .is_err()
    );
    let empty = santi_core::soul::Identity {
        name: " ".into(),
        ..identity.clone()
    };
    assert!(
        harness
            .service
            .clone()
            .identified([(santi_core::GENESIS.into(), empty)].into())
            .await
            .is_err()
    );
    let empty = santi_core::soul::Identity {
        channels: [("github".into(), " ".into())].into(),
        ..identity
    };
    assert!(
        harness
            .service
            .clone()
            .identified([(santi_core::GENESIS.into(), empty)].into())
            .await
            .is_err()
    );
    assert!(
        harness
            .service
            .soul(santi_core::GENESIS)
            .await
            .unwrap()
            .unwrap()
            .identity
            .is_none()
    );
}
