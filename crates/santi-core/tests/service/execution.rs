use super::support::*;
use santi_core::service::{self, Service};
use santi_core::{budget, message, strand};
use santi_provider::Call;
use serde_json::json;

#[derive(Clone, Default)]
struct BudgetedProvider {
    requests: Arc<Mutex<Vec<Request>>>,
    rounds: usize,
    command: Option<String>,
}

#[async_trait]
impl Provider for BudgetedProvider {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("output-provider"),
            model: "output-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let round = {
            let mut requests = self.requests.lock().unwrap();
            let round = requests.len();
            requests.push(request);
            round
        };
        if round < self.rounds {
            let command = self
                .command
                .clone()
                .unwrap_or_else(|| format!("printf '{}'", "x".repeat(100)));
            let arguments = json!({"command": command});
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Called(Call {
                    response: format!("response_{round}"),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: format!("call_{round}"),
                    name: "shell".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                Ok(Event::Completed {
                    response: Some(format!("response_{round}")),
                }),
            ])));
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("bounded output".to_string())),
            Ok(Event::Completed {
                response: Some("response_done".to_string()),
            }),
        ])))
    }
}

#[tokio::test]
async fn bounds() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(BudgetedProvider {
        rounds: 1,
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
        provider.clone(),
    )
    .await
    .expect("open service")
    .bounded(budget::Execution {
        profile: "test".to_string(),
        rounds: 3,
        calls: 4,
        output: 20,
        shell: 16,
    })
    .expect("bound service");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "capture output".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    let runtime = Probe::new(&service)
        .completed_turn(&strand.id, &accepted_turn(&posted).id)
        .await;
    assert_eq!(runtime.results.len(), 1);
    let output = runtime.results[0].output.as_ref().expect("tool output");
    assert_eq!(output["output_truncated"], true);
    assert_eq!(output["output_limit_bytes"], 16);
    assert_eq!(output["stdout"].as_str().unwrap().len(), 16);
    let snapshot = service
        .audit(&strand.id)
        .await
        .expect("read budget")
        .expect("budget snapshot");
    assert_eq!(snapshot.execution.unwrap().profile, "test");
    assert_eq!(snapshot.usage.unwrap().output, 16);
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn normalizes() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let allotment = 8;
    let provider = Arc::new(BudgetedProvider {
        rounds: 1,
        command: Some(r"printf '\377\377\377\377'; printf '\376\376\376\376' >&2".to_string()),
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
        provider.clone(),
    )
    .await
    .expect("open service")
    .bounded(budget::Execution {
        profile: "test".to_string(),
        rounds: 3,
        calls: 4,
        output: 100,
        shell: allotment,
    })
    .expect("bound service");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "capture invalid output".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    let runtime = Probe::new(&service)
        .completed_turn(&strand.id, &accepted_turn(&posted).id)
        .await;
    assert_eq!(runtime.results.len(), 1);
    let output = runtime.results[0].output.as_ref().expect("tool output");
    let stdout = output["stdout"].as_str().expect("stdout string");
    let stderr = output["stderr"].as_str().expect("stderr string");
    let captured = stdout.len() + stderr.len();
    assert!(captured <= allotment);
    assert!(stdout.contains('\u{fffd}') || stderr.contains('\u{fffd}'));
    assert_eq!(output["output_truncated"], true);
    assert_eq!(output["output_limit_bytes"], allotment);
    let snapshot = service
        .audit(&strand.id)
        .await
        .expect("read budget")
        .expect("budget snapshot");
    let usage = snapshot.usage.expect("budget usage");
    assert_eq!(usage.output, captured);
    assert!(usage.output <= allotment);
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

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

#[tokio::test]
async fn rounds() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(BudgetedProvider {
        rounds: usize::MAX,
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
        provider.clone(),
    )
    .await
    .expect("open service")
    .bounded(budget::Execution {
        profile: "test".to_string(),
        rounds: 2,
        calls: 4,
        output: 100,
        shell: 20,
    })
    .expect("bound service");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "keep calling".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    let runtime = Probe::new(&service)
        .failed_turn(&strand.id, &accepted_turn(&posted).id)
        .await;
    assert_eq!(runtime.results.len(), 1);
    let incident = runtime
        .errors
        .iter()
        .find(|incident| incident.code == "runtime.execution_budget.exceeded")
        .expect("execution incident");
    assert_eq!(incident.latest.context["reason"], "provider_rounds");
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}
