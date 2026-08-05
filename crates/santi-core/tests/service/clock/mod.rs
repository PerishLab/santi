use super::support::*;
use santi_core::service::{self, Service};
use santi_core::{message, soul, strand};

mod settlement;

#[derive(Clone, Default)]
struct Holding {
    requests: Arc<Mutex<Vec<Request>>>,
    complete: bool,
}

#[async_trait]
impl Provider for Holding {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("clock-provider"),
            model: "clock-model".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let clock = request.input.iter().any(|item| {
            matches!(item, Item::Message { content, .. } if content.contains("current_time:"))
        });
        self.requests.lock().unwrap().push(request);
        if clock && self.complete {
            return Ok(Box::pin(stream::iter(vec![
                Ok(Event::Text("clock heard".to_string())),
                Ok(Event::Completed {
                    response: Some("clock-response".to_string()),
                }),
            ])));
        }
        Ok(Box::pin(stream::pending()))
    }
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
async fn rings() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let provider = Arc::new(Holding {
        complete: true,
        ..Default::default()
    });
    let service = Service::open(config(&temp), provider.clone())
        .await
        .expect("open service")
        .clock(Duration::from_secs(60))
        .expect("set clock");
    let service = service
        .window(Duration::from_secs(1))
        .expect("set observation window");
    let idle = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken idle soul");
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
    assert_eq!(clock.soul, strand.soul);
    assert_ne!(clock.soul, idle.id);
    assert!(
        !service
            .strands()
            .await
            .expect("list strands")
            .iter()
            .any(|candidate| candidate.soul == idle.id
                && candidate.label.as_deref() == Some("santi:clock"))
    );
    let runtime = Probe::new(&service).any_completed(&clock.id).await;
    let pulse = runtime
        .messages
        .iter()
        .find(|message| message.text.contains("current_time:"))
        .expect("clock pulse");
    assert!(pulse.text.contains("wake: true"));
    assert!(!pulse.text.contains("job_id"));
    assert!(!pulse.text.contains("recommend"));
    assert!(!pulse.text.contains("diagnos"));

    service
        .stop(&accepted_turn(&posted).id)
        .await
        .expect("stop held turn");
    service.close();
    watcher.await.expect("watcher");
}

#[tokio::test]
async fn coalesces() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let database = temp.path().join("santi.sqlite");
    let service = Service::open(config(&temp), Arc::new(Holding::default()))
        .await
        .expect("open service")
        .clock(Duration::from_secs(1))
        .expect("set clock")
        .window(Duration::from_secs(60))
        .expect("set observation window");
    let strand = service.weave().await.expect("create strand").strand;
    service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "hold through several pulses".to_string(),
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
    sleep(Duration::from_millis(2200)).await;
    let store = santi_core::Store::open(&database)
        .await
        .expect("open store");
    let pending = store.inboxes(&clock.id).await.expect("pending pulses");
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].coalesce_key.as_deref(),
        Some(format!("clock/{}", strand.soul).as_str())
    );
    assert!(pending[0].content.rendered().contains("current_time:"));

    service.quiesce(Duration::ZERO);
    watcher.await.expect("watcher");
    service.drain().await;
}

#[tokio::test]
async fn projects() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = Service::open(config(&temp), Arc::new(Holding::default()))
        .await
        .expect("open service")
        .bounded(santi_core::budget::Execution {
            profile: "test".to_string(),
            rounds: 16,
            calls: 256,
            output: 4096,
            shell: 1024,
        })
        .expect("bound service");
    let soul = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken soul");
    let mut held = Vec::new();
    for index in 0..18 {
        let strand = service.seat(&soul.id).await.expect("seat soul").strand;
        let posted = service
            .send(
                &strand.id,
                strand::Post {
                    content: vec![message::Part::Text {
                        text: format!("hold {index}"),
                    }],
                },
            )
            .await
            .expect("send held turn");
        held.push((strand, accepted_turn(&posted).id.clone()));
    }
    let foreign = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken foreign soul");
    let strand = service
        .seat(&foreign.id)
        .await
        .expect("foreign seat")
        .strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "hold foreign".to_string(),
                }],
            },
        )
        .await
        .expect("send foreign turn");
    let foreign = (strand.id, accepted_turn(&posted).id.clone());
    let excluded = held[0].0.id.clone();
    let mut projection = None;
    for _ in 0..200 {
        let current = service
            .running(&soul.id, Some(&excluded))
            .await
            .expect("project turns")
            .expect("known soul");
        if current.total == 17 {
            projection = Some(current);
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    let projection = projection.expect("turns became active");
    assert_eq!(projection.total, 17);
    assert!(projection.truncated);
    assert_eq!(projection.active.len(), 16);
    for entry in &projection.active {
        assert_ne!(entry.strand.id, excluded);
        assert_ne!(entry.strand.id, foreign.0);
        assert_eq!(entry.turn.status, santi_core::turn::Status::Running);
        assert_eq!(entry.execution.as_ref().expect("budget").rounds, 16);
        assert!(entry.usage.is_some());
    }
    assert!(projection.active.windows(2).all(|pair| {
        (&pair[0].turn.created, &pair[0].turn.id) <= (&pair[1].turn.created, &pair[1].turn.id)
    }));
    assert!(
        service
            .running("soul_missing", None)
            .await
            .expect("missing soul")
            .is_none()
    );
    for (_, turn) in held {
        service.stop(&turn).await.expect("stop turn");
    }
    service.stop(&foreign.1).await.expect("stop foreign turn");
    service.quiesce(Duration::ZERO);
    service.drain().await;
}
