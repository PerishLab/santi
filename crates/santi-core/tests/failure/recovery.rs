use super::*;

#[tokio::test]
async fn recovers() {
    let temp = tempfile::tempdir().expect("temp");
    let provider = Arc::new(FailureProvider {
        fail_with: Some("temporary provider outage".to_string()),
        fail_for_requests: Some(1),
        ..FailureProvider::default()
    });
    let service = open_service(&temp, provider.clone()).await;
    let strand = service.weave().await.expect("strand").strand;
    let accepted = send_text(&service, &strand.id, "recover original accepted text").await;
    wait_for_turn(
        &service,
        &strand.id,
        &turn(&accepted).id,
        turn::Status::Failed,
    )
    .await;
    let prior = service
        .receipt(&accepted.receipt.inbox)
        .await
        .expect("receipt")
        .expect("held");
    service.resume().await.expect("automatic resume");
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    let driven = service.drive(&strand.id).await.expect("explicit drive");
    assert_eq!(driven.state, santi_core::drive::State::Started);
    let retried = driven.turn.expect("new turn");
    assert_ne!(retried.id, turn(&accepted).id);
    let runtime = wait_for_turn(&service, &strand.id, &retried.id, turn::Status::Completed).await;
    let status = service
        .receipt(&accepted.receipt.inbox)
        .await
        .expect("receipt")
        .expect("held");
    assert_eq!(status.state, santi_core::receipt::State::Completed);
    assert_eq!(status.accepted, prior.accepted);
    assert_eq!(
        runtime
            .messages
            .iter()
            .filter(|placed| placed.text == "recover original accepted text")
            .count(),
        1
    );
    assert!(runtime.effects.is_empty());
    assert_eq!(
        service
            .drive(&strand.id)
            .await
            .expect("settled drive")
            .state,
        santi_core::drive::State::Idle
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}
