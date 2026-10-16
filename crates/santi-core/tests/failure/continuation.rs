use super::*;
use santi_provider::Call;
use serde_json::json;

struct Continuation {
    requests: Mutex<Vec<Request>>,
    command: String,
}

#[async_trait]
impl Provider for Continuation {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("continuation"),
            model: "continuation".into(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let round = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len()
        };
        if round == 2 {
            return Err("provider unavailable after shell execution".into());
        }
        let event = if round == 1 {
            let arguments = json!({"command": self.command});
            Event::Called(Call {
                response: "response_effect".into(),
                mark: None,
                item: json!({"type": "function_call"}),
                call: "call_effect".into(),
                name: "shell".into(),
                raw: arguments.to_string(),
                arguments,
            })
        } else {
            Event::Text("continued with retained shell result".into())
        };
        Ok(Box::pin(stream::iter(vec![
            Ok(event),
            Ok(Event::Completed {
                response: Some(format!("response_{round}")),
            }),
        ])))
    }
}

#[tokio::test]
async fn continues() {
    let temp = tempfile::tempdir().unwrap();
    let marker = temp.path().join("applied");
    let provider = Arc::new(Continuation {
        requests: Mutex::new(Vec::new()),
        command: format!("printf applied >> '{}'; printf retained", marker.display()),
    });
    santi_core::Store::bootstrap(
        temp.path().join("santi.sqlite"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    let service = open(&temp, provider.clone()).await;
    let strand = service.weave().await.unwrap().strand;
    let original = send_text(&service, &strand.id, "perform one external action").await;
    let prior = turn(&service, &original).await.id;
    let failed = wait_for_turn(&service, &strand.id, &prior, turn::Status::Failed).await;
    assert_eq!(failed.calls.len(), 1);
    assert_eq!(failed.effects.len(), 1);
    assert_eq!(
        failed.results[0].output.as_ref().unwrap()["stdout"],
        "retained"
    );
    let receipt = service
        .receipt(&original.receipt.inbox)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(receipt.state, santi_core::receipt::State::Failed);
    let refusal = service.drive(&strand.id).await.unwrap_err();
    assert_eq!(refusal.context["recovery"]["state"], "settled_applied");
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    assert!(service.degraded());
    drop(service);

    let service = open(&temp, provider.clone()).await;
    service.resume().await.unwrap();
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
    let fresh = send_text(&service, &strand.id, "continue from the retained result").await;
    let resumed = turn(&service, &fresh).await.id;
    let completed = wait_for_turn(&service, &strand.id, &resumed, turn::Status::Completed).await;
    assert_eq!(std::fs::read_to_string(marker).unwrap(), "applied");
    assert_eq!(completed.calls.len(), 1);
    assert_eq!(completed.effects.len(), 1);
    assert_eq!(completed.results.len(), 1);
    assert_eq!(
        serde_json::to_value(&receipt).unwrap(),
        serde_json::to_value(
            service
                .receipt(&original.receipt.inbox)
                .await
                .unwrap()
                .unwrap()
        )
        .unwrap()
    );
    assert!(!service.degraded());
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(
        requests[2]
            .input
            .iter()
            .any(|item| { matches!(item, Item::Call { .. }) })
    );
    assert!(requests[2].input.iter().any(|item| {
        as_text(item).is_some_and(|(_, text)| text.contains("continue from the retained result"))
    }));
    assert!(requests[2].input.iter().any(|item| {
        matches!(item, Item::Output { output, .. } if output.contains("retained"))
    }));
}

async fn open(temp: &tempfile::TempDir, provider: Arc<Continuation>) -> Service {
    Service::open(
        service::Config {
            database: temp.path().join("santi.sqlite").display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".into()),
            constitution: None,
            environment: Default::default(),
        },
        provider,
    )
    .await
    .unwrap()
}
