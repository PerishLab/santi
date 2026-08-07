use super::*;
use serde_json::Value;

#[derive(Clone, Default)]
struct Fixture {
    requests: Arc<Mutex<Vec<Request>>>,
}

fn called(round: usize, call: String, name: &str, arguments: Value) -> Result<Event, String> {
    Ok(Event::Called(Call {
        response: format!("response_{round}"),
        mark: None,
        item: json!({"type": "function_call"}),
        call,
        name: name.to_string(),
        raw: arguments.to_string(),
        arguments,
    }))
}

fn observations(round: usize) -> Vec<Result<Event, String>> {
    let premature = json!({
        "kind": "native",
        "command": "printf premature-feedback"
    });
    let mut events = vec![called(
        round,
        "call_premature_feedback".to_string(),
        "feedback",
        premature,
    )];
    events.extend((0..3).map(|index| {
        called(
            round,
            format!("call_observe_{index}"),
            "shell",
            json!({"command": format!("printf observe-{index}")}),
        )
    }));
    events
}

#[async_trait]
impl Provider for Fixture {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("feedback-provider"),
            model: "feedback-model".to_string(),
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
        let mut events = Vec::new();
        match round {
            0 => {
                events.extend(observations(round));
                events.push(Ok(Event::Completed {
                    response: Some(format!("response_{round}")),
                }));
            }
            1 => {
                let arguments = json!({"command": "printf invalid-feedback"});
                events.push(Ok(Event::Called(Call {
                    response: format!("response_{round}"),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: "call_invalid_feedback".to_string(),
                    name: "feedback".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })));
                events.push(Ok(Event::Completed {
                    response: Some(format!("response_{round}")),
                }));
            }
            2 => {
                let arguments = json!({
                    "kind": "native",
                    "command": "printf native-feedback"
                });
                events.push(Ok(Event::Called(Call {
                    response: format!("response_{round}"),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: "call_feedback".to_string(),
                    name: "feedback".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })));
                events.push(Ok(Event::Completed {
                    response: Some(format!("response_{round}")),
                }));
            }
            _ => {
                events.push(Ok(Event::Text("barrier discharged".to_string())));
                events.push(Ok(Event::Completed {
                    response: Some("response_done".to_string()),
                }));
            }
        }
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
async fn barrier() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(Fixture::default());
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
        profile: "feedback-test".to_string(),
        rounds: 6,
        calls: 8,
        output: 1024,
        shell: 256,
        feedback_after_calls: Some(2),
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
                    text: "stop touring and obtain feedback".to_string(),
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
            "feedback", "shell", "shell", "shell", "feedback", "feedback"
        ]
    );
    assert_eq!(runtime.results.len(), 6);
    assert_eq!(
        runtime.results[0].error.as_deref(),
        Some("unsupported tool: feedback")
    );
    assert_eq!(
        runtime.results[1].output.as_ref().unwrap()["stdout"],
        "observe-0"
    );
    assert_eq!(
        runtime.results[2].output.as_ref().unwrap()["stdout"],
        "observe-1"
    );
    assert!(
        runtime.results[3]
            .error
            .as_deref()
            .expect("barrier error")
            .contains("feedback barrier reached after 2 ordinary shell calls")
    );
    assert!(
        runtime.results[4]
            .error
            .as_deref()
            .expect("invalid feedback error")
            .contains("missing field `kind`")
    );
    assert_eq!(
        runtime.results[5].output.as_ref().unwrap()["stdout"],
        "native-feedback"
    );
    assert_eq!(runtime.effects.len(), 4);

    {
        let requests = provider.requests.lock().unwrap();
        assert_eq!(names(&requests[0]), vec!["shell"]);
        assert_eq!(names(&requests[1]), vec!["feedback", "wake"]);
        assert_eq!(names(&requests[2]), vec!["feedback", "wake"]);
        assert_eq!(names(&requests[3]), vec!["shell"]);
    }
    let snapshot = service
        .audit(&strand.id)
        .await
        .expect("read budget")
        .expect("budget snapshot");
    assert_eq!(snapshot.execution.unwrap().feedback_after_calls, Some(2));
}
