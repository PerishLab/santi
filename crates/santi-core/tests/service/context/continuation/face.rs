use super::*;

#[tokio::test]
async fn refused() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;

    let provider = Arc::new(Script::new(vec![tools(), finish("answer")]));
    let service = Service::open(config(&temp), provider).await.unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let messages = service.strand(&strand.id).await.unwrap().unwrap().messages;
    assert!(messages.len() >= 3, "the tool round must be recorded");

    let store = santi_core::Store::open(config(&temp).database)
        .await
        .unwrap();
    let before = store.compacts(&strand.id).await.unwrap().len();

    let refused = service
        .exec(
            &strand.id,
            compact::Exec {
                first: Some(messages[0].message.id.clone()),
                last: Some(messages[1].message.id.clone()),
                from: None,
                to: None,
                summary: "cut the tool round".into(),
                absorb: Vec::new(),
                capsule: None,
                dry: false,
            },
        )
        .await
        .expect_err("an unsafe explicit range must be refused before commit");
    assert!(
        refused.contains("compact.anchor") || refused.contains("compact.exchange"),
        "the refusal must name the structural conflict: {refused}"
    );
    assert_eq!(
        store.compacts(&strand.id).await.unwrap().len(),
        before,
        "a refused explicit range leaves Compact unchanged"
    );
}
