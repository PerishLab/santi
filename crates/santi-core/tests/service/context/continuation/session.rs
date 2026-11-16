use super::*;

#[tokio::test]
async fn identity() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let provider = Arc::new(Script::new(vec![
        tools(),
        finish("first"),
        finish("second"),
    ]));
    let service = Service::open(config(&temp), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    send(&service, &strand.id).await;
    let messages = service.strand(&strand.id).await.unwrap().unwrap().messages;
    service
        .exec(
            &strand.id,
            compact::Exec {
                first: Some(messages.first().unwrap().message.id.clone()),
                last: Some(messages.last().unwrap().message.id.clone()),
                from: None,
                to: None,
                summary: "The test tools completed.".into(),
                absorb: Vec::new(),
                capsule: None,
                dry: false,
            },
        )
        .await
        .unwrap();
    send(&service, &strand.id).await;
    let requests = provider.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    assert!(
        requests
            .iter()
            .all(|request| request.strand.as_deref() == Some(strand.id.as_str()))
    );
    drop(service);

    let next = Arc::new(Script::new(vec![finish("restart"), finish("fork")]));
    let service = Service::open(config(&temp), next.clone()).await.unwrap();
    send(&service, &strand.id).await;
    let child = service.fork(&strand.id).await.unwrap().strand;
    send(&service, &child.id).await;
    let requests = next.requests.lock().unwrap();
    assert_eq!(requests[0].strand.as_deref(), Some(strand.id.as_str()));
    assert_eq!(requests[1].strand.as_deref(), Some(child.id.as_str()));
    assert_ne!(strand.id, child.id);
}
