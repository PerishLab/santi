use super::*;

struct Ran {
    temp: tempfile::TempDir,
    strand: String,
    call: String,
    output: serde_json::Value,
    error: Option<String>,
    effect: santi_core::effect::Effect,
}

async fn ran(command: &str, shell: usize) -> Ran {
    bounded(command, shell, 60).await
}

async fn bounded(command: &str, shell: usize, timeout: u64) -> Ran {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
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
    .expect("open service")
    .bounded(budget::Execution {
        timeout,
        profile: "test".to_string(),
        rounds: 3,
        calls: 4,
        output: 100_000,
        shell,
        lines: 8,
        feedback_after_calls: None,
        feedback_command: None,
        feedback_cwd: None,
    })
    .expect("bound service");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "capture bounded output".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    let turn = accepted_turn(&service, &posted).await;
    let runtime = tokio::time::timeout(
        Duration::from_secs(5),
        completion(&service, &strand.id, &turn.id),
    )
    .await
    .expect("bounded shell turn must finish");
    Ran {
        error: runtime.results[0].error.clone(),
        effect: runtime.effects[0].clone(),
        temp,
        strand: strand.id,
        call: runtime.calls[0].id.clone(),
        output: runtime.results[0].output.clone().unwrap_or_else(|| {
            let error: serde_json::Value =
                serde_json::from_str(runtime.results[0].error.as_deref().unwrap()).unwrap();
            error["output"].clone()
        }),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn lines() {
    let ran = ran("seq 1 100", 4096).await;
    let stdout = ran.output["stdout"].as_str().expect("stdout");
    assert_eq!(ran.output["output_truncated"], true);
    assert_eq!(ran.output["output_limit_lines"], 8);
    assert!(stdout.starts_with("1\n2\n3\n4\n[… "));
    assert!(stdout.contains(" bytes, 92 lines omitted; full output: strand://outputs/"));
    assert!(stdout.ends_with("…]\n97\n98\n99\n100\n"));
}

#[cfg(unix)]
#[tokio::test]
async fn streams() {
    let ran = ran(
        "head -c 5000 /dev/zero | tr '\\0' a; printf 'tail-of-stderr' >&2",
        64,
    )
    .await;
    let stdout = ran.output["stdout"].as_str().expect("stdout");
    let stderr = ran.output["stderr"].as_str().expect("stderr");
    assert_eq!(stderr, "tail-of-stderr");
    assert!(stdout.contains("bytes, 0 lines omitted"));
    assert!(ran.output["saved"].get("stderr").is_none());
}

#[cfg(unix)]
#[tokio::test]
async fn saved() {
    let command = r"printf 'token=fixture-token\n'; head -c 3000 /dev/zero | tr '\0' b";
    let ran = ran(command, 64).await;
    let uri = ran.output["saved"]["stdout"].as_str().expect("saved uri");
    let name = format!("outputs/{}.stdout.txt", ran.call);
    assert_eq!(uri, format!("strand://{name}"));
    assert!(ran.output["stdout"].as_str().unwrap().contains(uri));
    let path = ran
        .temp
        .path()
        .join("runtime/strands")
        .join(&ran.strand)
        .join("memory")
        .join(&name);
    let text = std::fs::read_to_string(path).expect("saved output");
    assert!(text.contains("[redacted]"));
    assert!(!text.contains("fixture-token"));
    assert!(text.ends_with(&"b".repeat(3000)));
}

#[tokio::test]
async fn deadline() {
    let ran = bounded("printf partial; sleep 30", 1024, 1).await;
    assert_eq!(ran.output["timed_out"], true);
    assert_eq!(ran.output["stdout"], "partial");
    assert!(
        ran.error
            .as_deref()
            .unwrap()
            .contains("partial external effects")
    );
    assert_eq!(ran.effect.state, santi_core::effect::State::Unknown);
    assert!(ran.effect.result.is_some());
    assert!(ran.effect.settled.is_none());
}

async fn completion(service: &Service, strand: &str, turn: &str) -> santi_core::stream::Snapshot {
    loop {
        let runtime = service.snapshot(strand).await.unwrap().unwrap();
        let state = runtime
            .turns
            .iter()
            .find(|held| held.id == turn)
            .unwrap()
            .status
            .clone();
        assert_ne!(
            state,
            santi_core::turn::Status::Failed,
            "{:?}",
            runtime.errors
        );
        if state == santi_core::turn::Status::Completed {
            return runtime;
        }
        sleep(Duration::from_millis(20)).await;
    }
}
