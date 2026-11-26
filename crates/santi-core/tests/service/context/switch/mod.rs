use super::super::support::*;
use santi_core::inference::{Declaration, Effective, Origin, Patch, Source};
use santi_core::provider::Resolver;
use santi_core::service::{self, Service, jobs};
use santi_core::{Store, message};
use santi_estate::{Target, Turn};
use tokio::sync::Notify;

struct Held {
    provider: &'static str,
    model: &'static str,
    bytes: usize,
    maintenance: usize,
    entered: Arc<Notify>,
}

#[async_trait]
impl Provider for Held {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from(self.provider),
            model: self.model.to_string(),
            budget: Some(santi_provider::Cap {
                bytes: self.bytes,
                maintenance: self.maintenance,
                source: "test".to_string(),
            }),
        }
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let _ = request;
        self.entered.notify_one();
        Ok(Box::pin(stream::pending::<Result<Event, String>>()))
    }
}

struct Switch {
    current: Mutex<Effective>,
    a: Arc<Held>,
    b: Arc<Held>,
}

impl Resolver for Switch {
    fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
        let _ = layers;
        Ok(self.current.lock().unwrap().clone())
    }

    fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
        match effective.selection.value().as_str() {
            "fixture/model/a" => Ok(self.a.clone()),
            "fixture/model/b" => Ok(self.b.clone()),
            other => Err(format!("unknown provider {other}")),
        }
    }
}

struct Jobs;

impl jobs::Supervisor for Jobs {
    fn detach(&self, launch: &jobs::Launch) -> Result<(), String> {
        let _ = launch;
        Err("jobs unavailable".to_string())
    }
    fn observe(&self, launch: &jobs::Launch) -> Result<jobs::Observation, String> {
        let _ = launch;
        Ok(jobs::Observation::Missing)
    }
    fn stop(&self, launch: &jobs::Launch) -> Result<(), String> {
        let _ = launch;
        Ok(())
    }
    fn acknowledge(&self, launch: &jobs::Launch) -> Result<(), String> {
        let _ = launch;
        Ok(())
    }
}

fn effective(provider: &str, model: &str, effort: &str) -> Effective {
    Effective {
        selection: santi_core::inference::Selection::Scenario(format!("fixture/model/{provider}")),
        model: model.to_string(),
        effort: Some(effort.to_string()),
        source: Source {
            route: None,
            selection: santi_core::inference::Selection::Scenario(Origin::Service),
            model: Origin::Service,
            effort: Some(Origin::Service),
        },
    }
}

#[tokio::test]
async fn missing() {
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
    let resolver = Arc::new(Switch {
        current: Mutex::new(effective("a", "a-model", "low")),
        a: Arc::new(Held {
            provider: "a",
            model: "a-model",
            bytes: 20_000,
            maintenance: 30_000,
            entered: Arc::new(Notify::new()),
        }),
        b: Arc::new(Held {
            provider: "b",
            model: "b-model",
            bytes: 40_000,
            maintenance: 60_000,
            entered: Arc::new(Notify::new()),
        }),
    });
    let service = Service::provided(
        config.clone(),
        resolver,
        Declaration::default(),
        Arc::new(Jobs),
    )
    .await
    .unwrap();
    let strand = service.weave().await.unwrap().strand;
    let store = Store::open(&config.database).await.unwrap();
    let from = store.strand(&strand.id).await.unwrap().unwrap().next;
    store
        .start(Turn {
            tag: "turn_unknown",
            strand: &strand.id,
            trigger: santi_core::turn::Trigger::System,
            from,
            created: &santi_core::now(),
        })
        .await
        .unwrap();

    let error = service
        .audit(&strand.id)
        .await
        .expect_err("a running turn without a snapshot must not borrow preview");
    assert!(error.contains("inference snapshot missing"), "{error}");
}

#[tokio::test]
async fn unbuildable() {
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
    let resolver = Arc::new(Switch {
        current: Mutex::new(effective("a", "a-model", "low")),
        a: Arc::new(Held {
            provider: "a",
            model: "a-model",
            bytes: 20_000,
            maintenance: 30_000,
            entered: Arc::new(Notify::new()),
        }),
        b: Arc::new(Held {
            provider: "b",
            model: "b-model",
            bytes: 40_000,
            maintenance: 60_000,
            entered: Arc::new(Notify::new()),
        }),
    });
    let service = Service::provided(
        config.clone(),
        resolver,
        Declaration::default(),
        Arc::new(Jobs),
    )
    .await
    .unwrap();
    let strand = service.weave().await.unwrap().strand;
    let store = Store::open(&config.database).await.unwrap();
    pinned(
        &store,
        &strand.id,
        Seed {
            tag: "turn_missing",
            text: "unknown provider",
            effective: &fixed("fixture/model/gone", "gone-model"),
        },
    )
    .await;

    let error = service
        .audit(&strand.id)
        .await
        .expect_err("a running turn whose provider cannot be rebuilt must not borrow preview");
    assert!(
        error.contains("unknown provider fixture/model/gone"),
        "{error}"
    );
    assert!(store.compacts(&strand.id).await.unwrap().is_empty());
    let messages = store.messages(&strand.id).await.unwrap();
    assert!(
        messages
            .iter()
            .any(|placed| { format!("{:?}", placed.message).contains("unknown provider") })
    );
}

mod selected;
