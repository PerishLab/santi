use super::*;
use santi_core::{message, soul, turn};

#[tokio::test]
async fn preserves() {
    let temp = tempfile::tempdir().expect("temp dir");
    let provider = Arc::new(Faulty {
        midstream: Some("provider stream aborted".to_string()),
        ..Faulty::default()
    });
    let service = open(&temp, provider.clone()).await;
    let soul = service
        .awaken(soul::Draft {
            memory: None,
            inference: Default::default(),
        })
        .await
        .expect("awaken soul");
    assert_ne!(soul.id, santi_core::GENESIS);
    let strand = service.seat(&soul.id).await.expect("seat soul").strand;
    let response = send(&service, &strand.id, "trigger stream failure").await;
    let failed = turn(&service, &response).await.id;

    let runtime = aborted(&service, &strand.id, &failed).await;
    let fragment = runtime
        .messages
        .iter()
        .find(|message| {
            message.message.role == message::Role::Soul
                && message.message.state == message::State::Aborted
        })
        .expect("aborted partial assistant message");
    assert_eq!(fragment.text, "partial runtime output");
    assert_eq!(fragment.message.actor, soul.id);
    unprojected(&runtime);
    assert_eq!(runtime.errors.len(), 1);
    assert_eq!(runtime.errors[0].first.source.operation, "turn.stream");
    assert_eq!(runtime.errors[0].first.context["stage"], "stream");

    let retry = send(&service, &strand.id, "continue with preserved partial").await;
    let failed = turn(&service, &retry).await.id;
    awaiting(&service, &strand.id, &failed, turn::Status::Failed).await;

    let projection = "partial runtime output";
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].input.iter().any(|message| {
        textual(message).is_some_and(|(role, content)| role == "assistant" && content == projection)
    }));
    assert!(requests[1].input.iter().all(|message| {
        textual(message).is_none_or(|(_, content)| !content.contains("kind: turn_failed"))
    }));
}

#[tokio::test]
async fn classifies() {
    let temp = tempfile::tempdir().expect("temp dir");
    let provider = Arc::new(Faulty {
        response: Some("provider rejected response".to_string()),
        ..Faulty::default()
    });
    let service = open(&temp, provider).await;
    let strand = service.weave().await.expect("create strand").strand;
    let response = send(&service, &strand.id, "trigger response failure").await;
    let failed = turn(&service, &response).await.id;

    let runtime = awaiting(&service, &strand.id, &failed, turn::Status::Failed).await;
    unprojected(&runtime);
    assert_eq!(runtime.errors.len(), 1);
    assert_eq!(runtime.errors[0].first.source.operation, "turn.response");
    assert_eq!(runtime.errors[0].first.context["stage"], "response");
}

#[tokio::test]
async fn resolves() {
    let temp = tempfile::tempdir().expect("temp dir");
    let provider = Arc::new(Faulty {
        failing: Some("temporary provider outage".to_string()),
        limit: Some(1),
        ..Faulty::default()
    });
    let service = open(&temp, provider.clone()).await;
    let strand = service.weave().await.expect("create strand").strand;
    let failed = send(&service, &strand.id, "first attempt").await;
    let original = turn(&service, &failed).await.id;
    let before = awaiting(&service, &strand.id, &original, turn::Status::Failed).await;
    let held = before.errors[0].id.clone();

    let recovered = send(&service, &strand.id, "retry after recovery").await;
    let resumed = turn(&service, &recovered).await.id;
    let after = awaiting(&service, &strand.id, &resumed, turn::Status::Completed).await;

    unprojected(&after);
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert_eq!(after.errors.len(), 1);
    let incident = &after.errors[0];
    assert_eq!(incident.id, held);
    assert_eq!(incident.status, santi_core::Status::Resolved);
    assert_eq!(incident.occurrences, 1);
    assert_eq!(incident.revision, 2);
    assert_eq!(
        incident.resolution.as_ref().unwrap().by.as_deref(),
        Some("provider.turn_succeeded")
    );
    assert_eq!(
        incident.latest.context["turn"],
        turn(&service, &recovered).await.id
    );
    assert_eq!(incident.latest.context["provider"], "fake-provider");
    assert_eq!(incident.latest.context["model"], "fake-model");
    assert_eq!(
        service
            .drive(&strand.id)
            .await
            .expect("covered history")
            .state,
        santi_core::drive::State::Idle
    );
    assert_eq!(
        service
            .receipt(&failed.receipt.inbox)
            .await
            .expect("old receipt")
            .expect("held")
            .state,
        santi_core::receipt::State::Failed
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}
