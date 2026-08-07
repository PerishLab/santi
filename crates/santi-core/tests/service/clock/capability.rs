use super::*;
use santi_provider::Call;
use serde_json::json;

#[derive(Clone, Default)]
struct Fixture;

#[async_trait]
impl Provider for Fixture {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("clock-shell-provider"),
            model: "clock-shell-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let clock = request.input.iter().any(|item| {
            matches!(item, Item::Message { content, .. } if content.contains("current_time:"))
        });
        if !clock {
            return Ok(Box::pin(stream::pending()));
        }
        if request
            .input
            .iter()
            .any(|item| matches!(item, Item::Output { .. }))
        {
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("clock tool rejected".to_string())),
                Ok(Event::Completed {
                    response: Some("clock-shell-done".to_string()),
                }),
            ])));
        }
        let arguments = json!({"command": "printf forbidden"});
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Called(Call {
                response: "clock-shell-call".to_string(),
                mark: None,
                item: json!({"type": "function_call"}),
                call: "call_clock_shell".to_string(),
                name: "shell".to_string(),
                raw: arguments.to_string(),
                arguments,
            })),
            Ok(Event::Completed {
                response: Some("clock-shell-call".to_string()),
            }),
        ])))
    }
}

#[tokio::test]
async fn rejects() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = Service::open(config(&temp), Arc::new(Fixture))
        .await
        .expect("open service")
        .clock(Duration::from_secs(60))
        .expect("set clock")
        .window(Duration::from_secs(1))
        .expect("set observation window");
    let strand = service.weave().await.expect("create strand").strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "hold this turn".to_string(),
                }],
            },
        )
        .await
        .expect("start held turn");
    let watcher = {
        let service = service.clone();
        tokio::spawn(async move { service.watch().await })
    };

    let mut clock = None;
    for _ in 0..150 {
        clock = service
            .strands()
            .await
            .expect("list strands")
            .into_iter()
            .find(|candidate| candidate.label.as_deref() == Some("santi:clock"));
        if clock.is_some() {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    let clock = clock.expect("clock strand");
    let runtime = Probe::new(&service).any_completed(&clock.id).await;
    assert_eq!(runtime.calls.len(), 1);
    assert_eq!(runtime.calls[0].tool, "shell");
    assert_eq!(runtime.results.len(), 1);
    assert_eq!(
        runtime.results[0].error.as_deref(),
        Some("unsupported tool on clock attention strand: shell")
    );
    assert!(runtime.effects.is_empty());

    service
        .stop(&accepted_turn(&posted).id)
        .await
        .expect("stop held turn");
    service.close();
    watcher.await.expect("watcher");
}
