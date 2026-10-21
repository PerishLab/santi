use super::*;
use santi_core::{message, strand};

#[tokio::test]
async fn locks() {
    let temp = tempfile::tempdir().expect("temp dir");
    super::support::bootstrap(&temp.path().join("santi.sqlite")).await;
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(BudgetedProvider),
    )
    .await
    .expect("open service");
    let strand = service.weave().await.expect("create strand").strand;

    let posted = send_strand_handler(
        State(service.clone()),
        Path(strand.id.clone()),
        Json(strand::Post {
            content: vec![message::Part::Text {
                text: "this exceeds the tiny budget".to_string(),
            }],
        }),
    )
    .await
    .unwrap_or_else(|error| panic!("send failed: {}: {}", error.code(), error.message()))
    .0;
    assert!(posted.turn.is_none());
    assert!(posted.message.is_none());
    let accepted = service
        .receipt(&posted.receipt.inbox)
        .await
        .expect("receipt lookup")
        .expect("accepted receipt");
    assert_eq!(accepted.state, santi_core::receipt::State::Accepted);

    let error = service
        .drive(&strand.id)
        .await
        .expect_err("drive held by budget");
    assert_eq!(error.code, "context.budget.exceeded");
    assert!(error.incident.is_some());
    assert!(error.message.contains("over budget"));
    assert!(!error.exposure.model);
    assert_eq!(
        service
            .receipt(&posted.receipt.inbox)
            .await
            .unwrap()
            .unwrap()
            .state,
        santi_core::receipt::State::Accepted
    );
}
