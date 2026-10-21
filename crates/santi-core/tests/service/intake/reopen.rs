use super::super::support::*;
use santi_core::message;
use santi_core::service::{self, Service};

#[tokio::test]
async fn resumes() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let config = service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: Some("127.0.0.1:0".to_string()),
        constitution: None,
        environment: Default::default(),
    };
    let provider = Arc::new(FakeProvider::default());
    let service = Service::open(config.clone(), provider.clone())
        .await
        .expect("open service");
    let strand = service.weave().await.expect("create strand").strand;
    service.close();
    let posted = service
        .send(
            &strand.id,
            santi_core::strand::Post {
                content: vec![message::Part::Text {
                    text: "restart accepted text".into(),
                }],
            },
        )
        .await
        .expect("durable send");
    assert!(posted.turn.is_none());
    assert!(posted.message.is_none());
    let receipt = service
        .receipt(&posted.receipt.inbox)
        .await
        .expect("receipt")
        .expect("accepted");
    assert_eq!(receipt.state, santi_core::receipt::State::Accepted);
    drop(service);

    let service = Service::open(config, provider.clone())
        .await
        .expect("reopen service");
    service.resume().await.expect("resume pending");
    let runtime = Probe::new(&service).any_completed(&strand.id).await;
    assert_eq!(runtime.turns.len(), 1);
    assert_eq!(runtime.calls.len(), 0);
    assert_eq!(runtime.effects.len(), 0);
    assert_eq!(
        runtime
            .messages
            .iter()
            .filter(|message| message.text == "restart accepted text")
            .count(),
        1
    );
    assert_eq!(
        service
            .receipt(&posted.receipt.inbox)
            .await
            .expect("receipt")
            .expect("completed")
            .state,
        santi_core::receipt::State::Completed
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
}
