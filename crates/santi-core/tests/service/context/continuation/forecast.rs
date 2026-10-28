use super::*;

#[tokio::test]
async fn forecasts() {
    for capsule in [false, true] {
        exercise(capsule).await;
    }
}

async fn exercise(capsule: bool) {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let mut provider = Script::new(vec![finish("settled")]);
    provider.limit = Some(500_000);
    let service = Service::open(config(&temp), Arc::new(provider))
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let messages = service.strand(&strand.id).await.unwrap().unwrap().messages;
    let first = messages.first().unwrap().message.id.clone();
    let last = messages.last().unwrap().message.id.clone();
    let request = compact::Exec {
        first: Some(first),
        last: Some(last),
        from: None,
        to: None,
        summary: "settled earlier exchange".into(),
        absorb: Vec::new(),
        capsule: capsule.then(|| compact::Capsule {
            source: "test".into(),
            reason: "make room".into(),
            risk: "exact wording omitted".into(),
            queryability: None,
        }),
        dry: true,
    };
    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    let preview = service.exec(&strand.id, request.clone()).await.unwrap();
    assert!(preview.dry && preview.after.is_some() && preview.ratio.is_some());
    assert!(store.compacts(&strand.id).await.unwrap().is_empty());
    let report = service
        .exec(
            &strand.id,
            compact::Exec {
                dry: false,
                ..request
            },
        )
        .await
        .unwrap();
    assert!(report.after.is_some());
    let records = store.compacts(&strand.id).await.unwrap();
    assert_eq!(records.len(), 1);
    let metadata = records[0].metadata.as_ref().unwrap();
    assert_eq!(
        metadata["before"]["total"],
        report.before.as_ref().unwrap().total
    );
    assert_eq!(metadata["forecast"]["authoritative"], false);
    assert_eq!(metadata["forecast"]["basis"], "precommit_preview");
    assert!(metadata["forecast"]["after"]["total"].is_number());
    assert!(metadata.get("after").is_none());
    assert_eq!(metadata.get("declared_source").is_some(), capsule);
    if capsule {
        assert_eq!(metadata["schema"], "santi.compact_capsule.v1");
    } else {
        assert_eq!(metadata["schema"], "santi.compact_estimate.v1");
        assert!(metadata.get("source_trust").is_none());
    }
    let input = santi_core::provider_input(&store, &strand.id)
        .await
        .unwrap();
    let Some(Item::Message { content, .. }) = input.first() else {
        panic!("compact header missing");
    };
    let header = content
        .split_once("[compact projection]\n")
        .unwrap()
        .1
        .split_once("\n[/compact projection]")
        .unwrap()
        .0;
    let header: serde_json::Value = serde_json::from_str(header).unwrap();
    let estimate = &header["context_estimate"];
    assert_eq!(estimate["pre_total_bytes"], metadata["before"]["total"]);
    assert_eq!(estimate["budget_input_bytes"], 500_000);
    assert_eq!(
        estimate["forecast"]["post_total_bytes"],
        metadata["forecast"]["after"]["total"]
    );
    assert_eq!(estimate["forecast"]["authoritative"], false);
    assert_eq!(estimate["availability"]["forecast"], "estimated");
    assert_eq!(
        store.compacts(&strand.id).await.unwrap()[0].metadata,
        records[0].metadata
    );
}
