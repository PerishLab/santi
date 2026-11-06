use super::super::layers::{Catalog, Jobs, Selection, declare};
use super::super::*;
use santi_core::inference::{Declaration, Effective, Origin};
use santi_core::provider::Resolver;
use santi_provider::Call;
use tokio::sync::Notify;

#[derive(Default, Clone, Copy)]
pub(super) enum Ending {
    #[default]
    Tools,
    Terminal,
    Failure,
}

#[derive(Default)]
pub(super) struct Rounds {
    ending: Ending,
    pub(super) requests: Mutex<Vec<(String, Option<String>, Request)>>,
    entered: [Notify; 2],
    pub(super) release: [Notify; 2],
}

struct Directory(Arc<Rounds>);

impl Resolver for Directory {
    fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
        Effective::resolve(layers, &Catalog)
    }

    fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
        Ok(Arc::new(Script {
            rounds: self.0.clone(),
            effective: effective.clone(),
        }))
    }
}

struct Script {
    rounds: Arc<Rounds>,
    effective: Effective,
}

#[async_trait]
impl Provider for Script {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from(self.effective.provider.as_str()),
            model: self.effective.model.clone(),
            budget: Some(santi_provider::Cap {
                bytes: 200_000,
                maintenance: 300_000,
                source: "test".into(),
            }),
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let index = {
            let mut requests = self.rounds.requests.lock().unwrap();
            let index = requests.len();
            requests.push((
                self.effective.provider.clone(),
                self.effective.reasoning_effort.clone(),
                request,
            ));
            index
        };
        if index < 2 {
            self.rounds.entered[index].notify_one();
            self.rounds.release[index].notified().await;
        }
        if index == 1 && matches!(self.rounds.ending, Ending::Failure) {
            return Err("after consumption".into());
        }
        if index == 0 && !matches!(self.rounds.ending, Ending::Terminal) {
            let mut events = Vec::new();
            for call in ["boundary_query", "second_query"] {
                let arguments = serde_json::json!({});
                events.push(Ok(Event::Called(Call {
                    response: "first".into(),
                    mark: None,
                    item: serde_json::json!({}),
                    call: call.into(),
                    name: "query".into(),
                    raw: arguments.to_string(),
                    arguments,
                })));
            }
            events.push(Ok(Event::Completed {
                response: Some("first".into()),
            }));
            return Ok(Box::pin(stream::iter(events)));
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Text("done".into())),
            Ok(Event::Completed {
                response: Some(format!("answer_{index}")),
            }),
        ])))
    }
}

pub(super) async fn entered(rounds: &Rounds, index: usize) {
    tokio::time::timeout(Duration::from_secs(10), rounds.entered[index].notified())
        .await
        .expect("request entered");
}

pub(super) async fn send(
    service: &Service,
    strand: &str,
    text: &str,
) -> santi_core::strand::Posted {
    service
        .send(
            strand,
            santi_core::strand::Post {
                content: vec![message::Part::Text { text: text.into() }],
            },
        )
        .await
        .unwrap()
}

pub(super) fn markers(request: &Request) -> usize {
    request
        .input
        .iter()
        .map(|item| match item {
            Item::Message { content, .. } => content.matches("unique_inbox_marker_369").count(),
            _ => 0,
        })
        .sum()
}

pub(super) struct Fixture {
    pub(super) service: Service,
    pub(super) rounds: Arc<Rounds>,
    pub(super) store: Store,
    pub(super) strand: String,
    pub(super) config: service::Config,
    _temp: tempfile::TempDir,
}
pub(super) async fn open(ending: Ending) -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    bootstrap(&temp).await;
    let config = service::Config {
        database: temp.path().join("santi.sqlite").display().to_string(),
        runtime: temp.path().join("runtime").display().to_string(),
        execution: temp.path().join("execution").display().to_string(),
        bind: None,
        constitution: None,
        environment: Default::default(),
    };
    let rounds = Arc::new(Rounds {
        ending,
        ..Default::default()
    });
    let service = Service::provided(
        config.clone(),
        Arc::new(Directory(rounds.clone())),
        Declaration::default(),
        Arc::new(Jobs),
    )
    .await
    .unwrap();
    let store = Store::open(&config.database).await.unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    declare(
        &store,
        &strand,
        Selection {
            provider: "alpha",
            model: "alpha-model",
            effort: "low",
        },
    )
    .await;
    Fixture {
        service,
        rounds,
        store,
        strand,
        config,
        _temp: temp,
    }
}
