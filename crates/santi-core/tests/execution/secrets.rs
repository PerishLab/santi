use super::*;

#[tokio::test]
async fn secrets() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    #[cfg(unix)]
    let command = r#"printf '%s' '{"name":"owner","token":"fixture-token","nested":{"client_secret":"fixture-secret"},"credential_sha256":"digest"}'"#;
    #[cfg(windows)]
    let command = r#"Write-Output '{"name":"owner","token":"fixture-token","nested":{"client_secret":"fixture-secret"},"credential_sha256":"digest"}'"#;
    let provider = Arc::new(BudgetedProvider {
        rounds: 1,
        command: Some(command.to_string()),
        ..Default::default()
    });
    let service = Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        provider,
    )
    .await
    .expect("open service");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "inspect owner config".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    let runtime = Probe::new(&service)
        .completed_turn(&strand.id, &accepted_turn(&posted).id)
        .await;
    let output = runtime.results[0].output.as_ref().expect("tool output");
    let stdout = output["stdout"].as_str().expect("stdout");

    assert_eq!(output["redacted"], true);
    assert_eq!(output["redactions"], 2);
    assert!(stdout.contains("[redacted]"));
    assert!(stdout.contains("credential_sha256"));
    assert!(!stdout.contains("fixture-token"));
    assert!(!stdout.contains("fixture-secret"));
}
