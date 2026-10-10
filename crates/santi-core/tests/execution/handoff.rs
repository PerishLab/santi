use super::*;

#[tokio::test]
async fn continues() {
    verify(8, 32, 10000).await;
}

#[tokio::test]
async fn calls() {
    verify(64, 4, 10000).await;
}

#[tokio::test]
async fn output() {
    verify(64, 32, 800).await;
}

async fn verify(rounds: usize, calls: usize, output: usize) {
    let temp = tempfile::tempdir().expect("temp");
    bootstrap(&temp).await;
    let provider = Arc::new(BudgetedProvider {
        rounds: 10,
        maintenance: true,
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
        profile: "handoff".into(),
        rounds,
        calls,
        output,
        shell: output.min(1000),
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
                    text: "continue finite work".into(),
                }],
            },
        )
        .await
        .unwrap();
    assert!(posted.turn.is_some());
    let mut runtime = service.snapshot(&strand.id).await.unwrap().unwrap();
    for _ in 0..300 {
        if provider.requests.lock().unwrap().len() > 10
            && runtime
                .turns
                .last()
                .is_some_and(|turn| turn.status == santi_core::turn::Status::Completed)
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        runtime = service.snapshot(&strand.id).await.unwrap().unwrap();
    }
    assert!(runtime.turns.len() >= 2);
    assert!(
        runtime
            .turns
            .iter()
            .all(|turn| turn.status == santi_core::turn::Status::Completed)
    );
    assert_eq!(runtime.compacts.len(), runtime.turns.len() - 1);
    assert_eq!(
        runtime
            .calls
            .iter()
            .filter(|call| call.tool == "shell")
            .count(),
        10 - runtime.compacts.len()
    );
    assert!(runtime.effects.iter().all(|effect| effect.state
        == santi_core::effect::State::Settled(santi_core::effect::Outcome::Applied)));
    assert!(
        runtime
            .messages
            .iter()
            .any(|message| message.text.contains("kind: execution_pause"))
    );
    for turn in &runtime.turns {
        let calls = runtime
            .calls
            .iter()
            .filter(|call| call.turn == turn.id)
            .map(|call| call.id.as_str())
            .collect::<Vec<_>>();
        let spent = runtime
            .results
            .iter()
            .filter(|result| calls.contains(&result.call.as_str()))
            .map(|result| match &result.output {
                Some(value) if value.get("stdout").is_some() => {
                    value["stdout"].as_str().unwrap().len()
                        + value["stderr"].as_str().unwrap().len()
                }
                Some(value) => value.to_string().len(),
                None => result.error.as_ref().map_or(0, String::len),
            })
            .sum::<usize>();
        assert!(spent <= output, "{spent} exceeds {output}");
    }
    let usage = service
        .audit(&strand.id)
        .await
        .unwrap()
        .unwrap()
        .usage
        .unwrap();
    assert!(usage.calls < calls);
    let requests = provider.requests.lock().unwrap();
    let request = requests.iter().find(|request| request.tools.as_ref().unwrap().iter().any(|tool| matches!(tool, santi_provider::Tool::Function(function) if function.name == "compact"))).unwrap();
    let maintenance = serde_json::to_value(request.tools.as_ref().unwrap()).unwrap();
    assert_eq!(maintenance.as_array().unwrap().len(), 1);
    assert_eq!(maintenance[0]["Function"]["name"], "compact");
    assert!(request.input.iter().any(|item| matches!(item, Item::Message { content, .. } if content.contains("execution_maintenance"))));
}

#[tokio::test]
async fn confines() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let marker = temp.path().join("feedback-effect");
    let provider = Arc::new(BudgetedProvider {
        rounds: usize::MAX,
        second: Some("feedback".into()),
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
        provider,
    )
    .await
    .unwrap()
    .bounded(budget::Execution {
        profile: "confines".into(),
        rounds: 2,
        calls: 16,
        output: 10000,
        shell: 1000,
        feedback_after_calls: Some(1),
        feedback_command: Some(format!("touch '{}'", marker.display())),
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
    assert!(!marker.exists());
    assert_eq!(runtime.effects.len(), 1);
    assert!(
        runtime.results[1]
            .error
            .as_deref()
            .unwrap()
            .contains("unsupported tool")
    );
    assert!(
        !runtime
            .messages
            .iter()
            .any(|message| message.text.contains("kind: execution_pause"))
    );
}
