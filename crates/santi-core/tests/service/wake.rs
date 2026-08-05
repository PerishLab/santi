use super::support::*;
use santi_core::service::{self, Service};
use santi_core::{soul, wake};
use santi_provider::Call;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Default)]
struct Recorder {
    requests: Arc<Mutex<Vec<Request>>>,
}

#[async_trait]
impl Provider for Recorder {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("wake-provider"),
            model: "wake-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        self.requests.lock().unwrap().push(request);
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("wake heard".to_string())),
            Ok(Event::Completed {
                response: Some(format!(
                    "wake-response-{}",
                    self.requests.lock().unwrap().len()
                )),
            }),
        ])))
    }
}

#[tokio::test]
async fn decays() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let database = temp.path().join("santi.sqlite");
    let provider = Arc::new(Recorder::default());
    let service = open(&temp, provider.clone())
        .await
        .clock(Duration::from_millis(200))
        .expect("set clock")
        .window(Duration::from_millis(10))
        .expect("set window");
    let soul = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken soul");
    let dormant = service.wake(&soul.id).await.expect("status").expect("soul");
    assert_eq!(dormant.state, wake::State::Revoked);
    assert_eq!(dormant.generation, 0);
    let invited = service.enable_wake(&soul.id).await.expect("enable");
    assert_eq!(invited.state, wake::State::Active);
    assert_eq!(invited.rounds_remaining, 3);
    let scheduled = invited.next_at.clone().expect("first schedule");
    drop(service);

    sleep(Duration::from_millis(700)).await;
    let service = Service::open(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        provider.clone(),
    )
    .await
    .expect("reopen service")
    .clock(Duration::from_millis(200))
    .expect("set clock")
    .window(Duration::from_millis(10))
    .expect("set window");
    let restored = service
        .wake(&soul.id)
        .await
        .expect("restored")
        .expect("soul");
    assert_eq!(restored.generation, invited.generation);
    assert_eq!(restored.rounds_remaining, 3);
    let watcher = {
        let service = service.clone();
        tokio::spawn(async move { service.watch().await })
    };

    waited(&provider, 1).await;
    let after = service
        .wake(&soul.id)
        .await
        .expect("after gap")
        .expect("soul");
    assert_eq!(after.rounds_remaining, 2);
    assert_ne!(after.last_wake_at.as_deref(), Some(scheduled.as_str()));
    let first = provider.requests.lock().unwrap()[0].clone();
    assert!(first.input.iter().any(
        |item| matches!(item, Item::Message { content, .. } if content.contains("current_time:") && !content.contains(&scheduled))
    ));
    for _ in 0..500 {
        let lease = service.wake(&soul.id).await.expect("poll").expect("soul");
        if lease.state == wake::State::Expired {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    let expired = service
        .wake(&soul.id)
        .await
        .expect("expired")
        .expect("soul");
    assert_eq!(expired.state, wake::State::Expired);
    assert_eq!(expired.rounds_remaining, 0);
    waited(&provider, 3).await;
    assert_eq!(provider.requests.lock().unwrap().len(), 3);
    sleep(Duration::from_millis(1_200)).await;
    assert_eq!(provider.requests.lock().unwrap().len(), 3);

    let renewed = service
        .renew_wake(&soul.id, expired.generation)
        .await
        .expect("renew final round");
    assert_eq!(renewed.rounds_remaining, 3);
    let revoked = service.disable_wake(&soul.id).await.expect("disable");
    assert_eq!(revoked.state, wake::State::Revoked);
    assert!(
        service
            .renew_wake(&soul.id, expired.generation)
            .await
            .is_err()
    );

    let requests = provider.requests.lock().unwrap().clone();
    assert!(requests.iter().all(|request| request.input.iter().any(
        |item| matches!(item, Item::Message { content, .. } if content.contains("item_kind: wake_lease"))
    )));
    service.close();
    watcher.await.expect("watcher");
}

async fn open(temp: &tempfile::TempDir, provider: Arc<Recorder>) -> Service {
    Service::open(
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
}

async fn waited(provider: &Recorder, count: usize) {
    for _ in 0..200 {
        if provider.requests.lock().unwrap().len() >= count {
            return;
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("wake request did not arrive");
}

#[derive(Clone, Default)]
struct Renewer {
    renewed: Arc<AtomicBool>,
}

#[async_trait]
impl Provider for Renewer {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("renewing-provider"),
            model: "renewing-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let wake = request.input.iter().any(
            |item| matches!(item, Item::Message { content, .. } if content.contains("item_kind: wake_lease")),
        );
        if wake && !self.renewed.swap(true, Ordering::SeqCst) {
            assert!(wake_tool(&request));
            let arguments = json!({"action": "renew", "generation": 1});
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Called(Call {
                    response: "response_wake".to_string(),
                    mark: None,
                    item: json!({"type": "function_call"}),
                    call: "call_wake_renew".to_string(),
                    name: "wake".to_string(),
                    raw: arguments.to_string(),
                    arguments,
                })),
                Ok(Event::Completed {
                    response: Some("response_wake".to_string()),
                }),
            ])));
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("renewed".to_string())),
            Ok(Event::Completed {
                response: Some("response_renewed".to_string()),
            }),
        ])))
    }
}

fn wake_tool(request: &Request) -> bool {
    request.tools.as_ref().is_some_and(|tools| {
        tools.iter().any(|tool| match tool {
            santi_provider::Tool::Function(tool) => {
                tool.name == "wake"
                    && tool
                        .description
                        .contains("caller-permitted autonomous wake lease")
                    && tool.description.contains("never renews a lease implicitly")
            }
        })
    })
}

#[tokio::test]
async fn renews() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(Renewer::default());
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
    .clock(Duration::from_secs(60))
    .expect("set clock")
    .window(Duration::from_millis(10))
    .expect("set window");
    let soul = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken soul");
    service.enable_wake(&soul.id).await.expect("enable");
    let watcher = {
        let service = service.clone();
        tokio::spawn(async move { service.watch().await })
    };
    for _ in 0..200 {
        if provider.renewed.load(Ordering::SeqCst) {
            let lease = service.wake(&soul.id).await.expect("status").expect("soul");
            if lease.rounds_remaining == 3 {
                break;
            }
        }
        sleep(Duration::from_millis(20)).await;
    }
    let lease = service
        .wake(&soul.id)
        .await
        .expect("renewed status")
        .expect("soul");
    assert!(provider.renewed.load(Ordering::SeqCst));
    assert_eq!(lease.state, wake::State::Active);
    assert_eq!(lease.rounds_remaining, 3);
    let clock = service
        .strands()
        .await
        .expect("strands")
        .into_iter()
        .find(|strand| strand.soul == soul.id && strand.label.as_deref() == Some("santi:clock"))
        .expect("clock strand");
    let runtime = Probe::new(&service).any_completed(&clock.id).await;
    assert!(runtime.results.iter().any(|result| {
        result
            .output
            .as_ref()
            .is_some_and(|output| output["state"] == "active" && output["rounds_remaining"] == 3)
    }));
    service.close();
    watcher.await.expect("watcher");
}
