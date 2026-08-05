use crate::support::*;
use santi_core::service::{self, Service};
use santi_core::{message, strand};
use santi_provider::Call;
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Clone, Default)]
struct Boundary {
    prior: Arc<AtomicUsize>,
    current: Arc<AtomicUsize>,
    released: Arc<AtomicBool>,
    rings: Arc<AtomicUsize>,
}

#[async_trait]
impl Provider for Boundary {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("boundary-provider"),
            model: "boundary-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let clock = request.input.iter().any(|item| {
            matches!(item, Item::Message { content, .. } if content.contains("current_time:"))
        });
        if clock {
            self.rings.fetch_add(1, Ordering::SeqCst);
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("clock heard".to_string())),
                Ok(Event::Completed {
                    response: Some("clock-response".to_string()),
                }),
            ])));
        }
        let current = request.input.iter().any(|item| {
            matches!(item, Item::Message { content, .. } if content.contains("hold the current turn"))
        });
        if current {
            while !self.released.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(10)).await;
            }
            let round = self.current.fetch_add(1, Ordering::SeqCst);
            return if round < 12 {
                Ok(call("current", round))
            } else {
                Ok(Box::pin(stream::pending()))
            };
        }
        let round = self.prior.fetch_add(1, Ordering::SeqCst);
        if round < 12 {
            Ok(call("prior", round))
        } else {
            Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("prior complete".to_string())),
                Ok(Event::Completed {
                    response: Some("prior-complete".to_string()),
                }),
            ])))
        }
    }
}

fn call(prefix: &str, round: usize) -> Streaming {
    let arguments = json!({"command": "true"});
    Box::pin(stream::iter(vec![
        Ok(Event::Called(Call {
            response: format!("response_{prefix}_{round}"),
            mark: None,
            item: json!({"type": "function_call"}),
            call: format!("call_{prefix}_{round}"),
            name: "shell".to_string(),
            raw: arguments.to_string(),
            arguments,
        })),
        Ok(Event::Completed {
            response: Some(format!("response_{prefix}_{round}")),
        }),
    ]))
}

fn config(temp: &tempfile::TempDir) -> service::Config {
    service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: Some("127.0.0.1:0".to_string()),
        constitution: None,
        environment: Default::default(),
    }
}

#[tokio::test]
async fn settles() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(Boundary::default());
    let service = Service::open(config(&temp), provider.clone())
        .await
        .expect("open service")
        .bounded(santi_core::budget::Execution {
            profile: "test".to_string(),
            rounds: 16,
            calls: 256,
            output: 4096,
            shell: 1024,
        })
        .expect("bound service")
        .clock(Duration::from_secs(60 * 60))
        .expect("set clock")
        .window(Duration::from_secs(60 * 60))
        .expect("set observation window");
    let strand = service.weave().await.expect("create strand").strand;
    let first = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "hold the prior turn".to_string(),
                }],
            },
        )
        .await
        .expect("start prior turn");
    let first = accepted_turn(&first).id.clone();
    Probe::new(&service)
        .completed_turn(&strand.id, &first)
        .await;

    let current = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "hold the current turn".to_string(),
                }],
            },
        )
        .await
        .expect("start current turn");
    let current = accepted_turn(&current).id.clone();
    let projection = service
        .running(&strand.soul, None)
        .await
        .expect("project active turn")
        .expect("known soul");
    assert_eq!(projection.active.len(), 1);
    assert_eq!(projection.active[0].turn.id, current);
    assert_eq!(
        projection.active[0].usage.as_ref().expect("usage").calls,
        12
    );

    let watcher = {
        let service = service.clone();
        tokio::spawn(async move { service.watch().await })
    };
    sleep(Duration::from_millis(1200)).await;
    assert!(
        service
            .strands()
            .await
            .expect("list strands")
            .iter()
            .all(|candidate| candidate.label.as_deref() != Some("santi:clock"))
    );

    provider.released.store(true, Ordering::SeqCst);
    let mut reached = false;
    for _ in 0..200 {
        let projection = service
            .running(&strand.soul, None)
            .await
            .expect("project active turn")
            .expect("known soul");
        reached = projection.active[0]
            .usage
            .as_ref()
            .is_some_and(|usage| usage.calls == 24);
        if reached {
            break;
        }
        sleep(Duration::from_millis(20)).await;
    }
    assert!(reached, "current turn should reach twelve direct calls");
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
    let clock = clock.expect("clock strand after current turn reaches boundary");
    Probe::new(&service).any_completed(&clock.id).await;
    sleep(Duration::from_millis(2200)).await;
    assert_eq!(provider.rings.load(Ordering::SeqCst), 1);

    service.stop(&current).await.expect("stop current turn");
    service.close();
    watcher.await.expect("watcher");
}
