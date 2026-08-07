use super::*;
use santi_core::STRANDSPACE;
use serde_json::Value;

#[derive(Clone, Default)]
struct Fixture {
    requests: Arc<Mutex<Vec<Request>>>,
}

fn called(round: usize, call: &str, name: &str, arguments: Value) -> Result<Event, String> {
    Ok(Event::Called(Call {
        response: format!("response_{round}"),
        mark: None,
        item: json!({"type": "function_call"}),
        call: call.to_string(),
        name: name.to_string(),
        raw: arguments.to_string(),
        arguments,
    }))
}

#[async_trait]
impl Provider for Fixture {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("caller-feedback-provider"),
            model: "caller-feedback-model".to_string(),
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
        let mut events = match round {
            0 => vec![
                called(
                    round,
                    "call_observe_0",
                    "shell",
                    json!({"command": "printf observe-0"}),
                ),
                called(
                    round,
                    "call_observe_1",
                    "shell",
                    json!({"command": "printf observe-1"}),
                ),
            ],
            1 => vec![called(
                round,
                "call_injected_feedback",
                "feedback",
                json!({"command": "printf model-owned"}),
            )],
            2 => vec![called(round, "call_red_feedback", "feedback", json!({}))],
            3 => vec![
                called(
                    round,
                    "call_observe_2",
                    "shell",
                    json!({"command": "printf observe-2"}),
                ),
                called(
                    round,
                    "call_observe_3",
                    "shell",
                    json!({"command": "printf observe-3"}),
                ),
            ],
            4 => vec![called(round, "call_green_feedback", "feedback", json!({}))],
            5 => vec![called(
                round,
                "call_reopened",
                "shell",
                json!({"command": "printf reopened"}),
            )],
            _ => vec![Ok(Event::Text("caller feedback completed".to_string()))],
        };
        events.push(Ok(Event::Completed {
            response: Some(format!("response_{round}")),
        }));
        Ok(Box::pin(stream::iter(events)))
    }
}

fn names(request: &Request) -> Vec<&str> {
    request
        .tools
        .as_ref()
        .expect("provider tools")
        .iter()
        .map(|tool| match tool {
            santi_provider::Tool::Function(tool) => tool.name.as_str(),
        })
        .collect()
}

#[tokio::test]
async fn authority() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(Fixture::default());
    #[cfg(unix)]
    let effect = "if [ -e caller-feedback.marker ]; then printf green; else : > caller-feedback.marker; printf red; exit 7; fi";
    #[cfg(windows)]
    let effect = "if (Test-Path caller-feedback.marker) { Write-Output -NoNewline green } else { New-Item caller-feedback.marker -ItemType File | Out-Null; Write-Output -NoNewline red; exit 7 }";
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
        profile: "caller-feedback-test".to_string(),
        rounds: 8,
        calls: 12,
        output: 4096,
        shell: 1024,
        feedback_after_calls: Some(2),
        feedback_command: Some(effect.to_string()),
        feedback_cwd: Some(STRANDSPACE.to_string()),
    })
    .expect("bound service");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "use only caller-owned feedback".to_string(),
                }],
            },
        )
        .await
        .expect("send strand");
    let runtime = Probe::new(&service)
        .completed_turn(&strand.id, &accepted_turn(&posted).id)
        .await;

    assert_eq!(
        runtime
            .calls
            .iter()
            .map(|call| call.tool.as_str())
            .collect::<Vec<_>>(),
        vec![
            "shell", "shell", "feedback", "feedback", "shell", "shell", "feedback", "shell"
        ]
    );
    assert_eq!(runtime.results.len(), 8);
    assert!(
        runtime.results[2]
            .error
            .as_deref()
            .expect("injected argument error")
            .contains("unknown field `command`")
    );
    assert_eq!(runtime.results[3].output.as_ref().unwrap()["stdout"], "red");
    assert_eq!(runtime.results[3].output.as_ref().unwrap()["exit_code"], 7);
    assert_eq!(
        runtime.results[6].output.as_ref().unwrap()["stdout"],
        "green"
    );
    assert_eq!(runtime.results[6].output.as_ref().unwrap()["exit_code"], 0);
    assert_eq!(
        runtime.results[7].output.as_ref().unwrap()["stdout"],
        "reopened"
    );

    {
        let requests = provider.requests.lock().unwrap();
        assert_eq!(requests.len(), 7);
        assert_eq!(names(&requests[0]), vec!["shell"]);
        assert_eq!(names(&requests[1]), vec!["feedback", "wake"]);
        assert_eq!(names(&requests[2]), vec!["feedback", "wake"]);
        assert_eq!(names(&requests[3]), vec!["shell"]);
        assert_eq!(names(&requests[4]), vec!["feedback", "wake"]);
        assert_eq!(names(&requests[5]), vec!["shell"]);
        let tools = requests[1].tools.as_ref().expect("provider tools");
        let santi_provider::Tool::Function(definition) = &tools[0];
        assert_eq!(definition.parameters["properties"], json!({}));
        assert_eq!(definition.parameters["additionalProperties"], false);
    }

    let execution = service
        .audit(&strand.id)
        .await
        .expect("read budget")
        .expect("budget snapshot")
        .execution
        .expect("execution budget");
    assert_eq!(execution.feedback_after_calls, Some(2));
    assert_eq!(execution.feedback_command.as_deref(), Some(effect));
    assert_eq!(execution.feedback_cwd.as_deref(), Some(STRANDSPACE));
}
