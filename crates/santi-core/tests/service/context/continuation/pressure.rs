use super::*;

#[tokio::test]
async fn pressure() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let mut events = tools();
    for event in &mut events {
        if let Event::Retained(state) = event {
            state.content = Some("r".repeat(100_000));
        }
    }
    let mut provider = Script::new(vec![events]);
    provider.limit = Some(40_000);
    let provider = Arc::new(provider);
    let service = Service::open(config(&temp), provider.clone())
        .await
        .unwrap();
    let strand = service.weave().await.unwrap().strand;
    service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "check reasoning budget".into(),
                }],
            },
        )
        .await
        .unwrap();
    let runtime = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let runtime = service.snapshot(&strand.id).await.unwrap().unwrap();
            if runtime
                .turns
                .iter()
                .any(|turn| turn.status == santi_core::turn::Status::Failed)
            {
                break runtime;
            }
            sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("reasoning budget must stop the turn");
    assert_eq!(
        provider.requests.lock().unwrap().len(),
        1,
        "oversize continuation must not reach another provider round"
    );
    assert!(!runtime.errors.is_empty());
}
