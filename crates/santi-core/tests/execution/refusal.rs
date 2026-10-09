use super::*;

#[tokio::test]
async fn incomplete() {
    verify(7, 8, 32, 1).await;
}

#[tokio::test]
async fn batch() {
    verify(2, 64, 4, 2).await;
}

async fn verify(work: usize, rounds: usize, calls: usize, batch: usize) {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let provider = Arc::new(BudgetedProvider {
        rounds: work,
        maintenance: true,
        batch,
        ..Default::default()
    });
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        provider.clone(),
    )
    .await
    .unwrap()
    .bounded(budget::Execution {
        profile: "incomplete".into(),
        rounds,
        calls,
        output: 10000,
        shell: 1000,
        feedback_after_calls: None,
        feedback_command: None,
        feedback_cwd: None,
    })
    .unwrap();
    let strand = service.weave().await.unwrap().strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "finite work".into(),
                }],
            },
        )
        .await
        .unwrap();
    let runtime = Probe::new(&service)
        .failed_turn(&strand.id, &accepted_turn(&posted).id)
        .await;
    assert_eq!(runtime.turns.len(), 1);
    assert!(runtime.compacts.is_empty());
    assert!(
        runtime
            .errors
            .iter()
            .any(|incident| incident.latest.context["reason"] == "compact_required")
    );
    assert!(
        !runtime
            .messages
            .iter()
            .any(|message| message.text.contains("kind: execution_pause"))
    );
    assert_eq!(provider.requests.lock().unwrap().len(), work + 1);
    let effects = if batch == 1 { work } else { batch };
    assert_eq!(runtime.effects.len(), effects);
    assert_eq!(runtime.calls.len(), effects);
}
