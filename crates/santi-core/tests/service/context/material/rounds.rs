use super::*;
use santi_provider::Call;

struct Paused {
    entered: Arc<tokio::sync::Notify>,
    release: Arc<tokio::sync::Notify>,
    requests: Arc<Mutex<Vec<Request>>>,
    failed: bool,
}

#[async_trait]
impl Provider for Paused {
    fn metadata(&self) -> Metadata {
        FakeProvider::default().metadata()
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let first = {
            let mut requests = self.requests.lock().unwrap();
            requests.push(request);
            requests.len() == 1
        };
        if self.failed {
            return Err("no headers received".into());
        }
        if first {
            self.entered.notify_one();
            self.release.notified().await;
            let arguments = serde_json::json!({});
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Called(Call {
                    response: "first".into(),
                    mark: None,
                    item: serde_json::json!({}),
                    call: "round_call".into(),
                    name: "query".into(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                Ok(Event::Completed {
                    response: Some("first".into()),
                }),
            ])));
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("done".into())),
            Ok(Event::Completed {
                response: Some("second".into()),
            }),
        ])))
    }
}

#[tokio::test]
async fn boundary() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    source(&temp, "round-marker");
    let entered = Arc::new(tokio::sync::Notify::new());
    let release = Arc::new(tokio::sync::Notify::new());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let provider = Paused {
        entered: entered.clone(),
        release: release.clone(),
        requests: requests.clone(),
        failed: false,
    };
    let service = Service::open(config(&temp), Arc::new(provider))
        .await
        .unwrap()
        .catalogued(sources(&temp))
        .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    service
        .load(Scope::Strand, &strand, command("load"))
        .await
        .unwrap();
    let posted = service
        .send(
            &strand,
            strand::Post {
                content: vec![santi_core::message::Part::Text {
                    text: "start".into(),
                }],
            },
        )
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .unwrap();
    service
        .drop(Scope::Strand, &strand, command("drop"))
        .await
        .unwrap();
    assert!(
        requests.lock().unwrap()[0]
            .instructions
            .as_ref()
            .unwrap()
            .contains("round-marker")
    );
    release.notify_one();
    let turn = accepted_turn(&service, &posted).await;
    let runtime = Probe::new(&service).completed_turn(&strand, &turn.id).await;
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(
        !requests[1]
            .instructions
            .as_ref()
            .unwrap()
            .contains("round-marker")
    );
    assert_eq!(runtime.contexts.len(), 2);
    assert_eq!(runtime.contexts[0].materials.len(), 1);
    assert!(runtime.contexts[1].materials.is_empty());
}

#[tokio::test]
async fn prepared() {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    source(&temp, "prepared-marker");
    let provider = Paused {
        entered: Arc::default(),
        release: Arc::default(),
        requests: Arc::default(),
        failed: true,
    };
    let service = Service::open(config(&temp), Arc::new(provider))
        .await
        .unwrap()
        .catalogued(sources(&temp))
        .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    service
        .load(Scope::Strand, &strand, command("load"))
        .await
        .unwrap();
    let posted = service
        .send(
            &strand,
            strand::Post {
                content: vec![santi_core::message::Part::Text {
                    text: "start".into(),
                }],
            },
        )
        .await
        .unwrap();
    let turn = accepted_turn(&service, &posted).await;
    let runtime = Probe::new(&service).failed_turn(&strand, &turn.id).await;
    assert_eq!(runtime.contexts.len(), 1);
    assert!(runtime.contexts[0].thinking.is_none());
    assert_eq!(runtime.contexts[0].materials.len(), 1);
}
