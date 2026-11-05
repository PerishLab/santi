use super::*;
use santi_core::inference::{Declaration, Defaults, Directory, Effective, Origin, Patch};
use santi_core::provider::Resolver;
use santi_core::service::{self, JobLaunch, JobObservation, JobSupervisor, Service};
use santi_estate::Target;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::watch;

#[derive(Clone, Debug)]
pub(crate) struct Call {
    pub(crate) kind: &'static str,
    pub(crate) provider: String,
    pub(crate) model: String,
    pub(crate) effort: Option<String>,
    pub(crate) text: String,
}

pub(crate) struct Catalog;

impl Directory for Catalog {
    fn defaults(&self, provider: &str) -> Result<Defaults, String> {
        Ok(Defaults {
            model: format!("{provider}-default"),
            reasoning_effort: None,
        })
    }
}

pub(crate) struct Fixture {
    _temp: tempfile::TempDir,
    pub(crate) service: Service,
    pub(crate) layers: Arc<Layers>,
    pub(crate) store: Store,
    pub(crate) strand: santi_core::strand::Strand,
}

pub(crate) struct Layers {
    pub(crate) calls: Arc<Mutex<Vec<Call>>>,
    pub(crate) gate: watch::Sender<bool>,
    pub(crate) switch: AtomicBool,
    pub(crate) builds: AtomicUsize,
    resolves: AtomicUsize,
    block: Arc<AtomicBool>,
}

#[async_trait]
impl Provider for Recorder {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from(self.provider.as_str()),
            model: self.model.clone(),
            budget: Some(santi_provider::Cap {
                bytes: self.bytes,
                maintenance: self.maintenance,
                source: "test".into(),
            }),
        }
    }

    fn estimate(&self, request: &Request) -> Option<santi_core::budget::Estimate> {
        self.calls.lock().unwrap().push(Call {
            kind: "estimate",
            provider: self.provider.clone(),
            model: request.model.clone(),
            effort: self.effort.clone(),
            text: String::new(),
        });
        Some(santi_core::budget::Estimate {
            estimator: "test".into(),
            items: 1,
            input: 1,
            instructions: 1,
            tools: 0,
            total: 3,
        })
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let text = request
            .input
            .iter()
            .filter_map(|item| match item {
                Item::Message { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.calls.lock().unwrap().push(Call {
            kind: "stream",
            provider: self.provider.clone(),
            model: request.model.clone(),
            effort: self.effort.clone(),
            text,
        });
        if self.block.load(Ordering::SeqCst) {
            let mut gate = self.gate.clone();
            let _ = gate.wait_for(|open| *open).await;
        }
        Ok(Box::pin(stream::iter(vec![
            Ok(Event::Started {
                response: Some(format!("{}-response", self.provider)),
            }),
            Ok(Event::Text("done".into())),
            Ok(Event::Completed {
                response: Some(format!("{}-response", self.provider)),
            }),
        ])))
    }
}

struct Recorder {
    provider: String,
    model: String,
    effort: Option<String>,
    bytes: usize,
    maintenance: usize,
    calls: Arc<Mutex<Vec<Call>>>,
    gate: watch::Receiver<bool>,
    block: Arc<AtomicBool>,
}

pub(crate) struct Jobs;

impl JobSupervisor for Jobs {
    fn detach(&self, launch: &JobLaunch) -> Result<(), String> {
        let _ = launch;
        Err("jobs unavailable".to_string())
    }
    fn observe(&self, launch: &JobLaunch) -> Result<JobObservation, String> {
        let _ = launch;
        Ok(JobObservation::Missing)
    }
    fn stop(&self, launch: &JobLaunch) -> Result<(), String> {
        let _ = launch;
        Ok(())
    }
    fn acknowledge(&self, launch: &JobLaunch) -> Result<(), String> {
        let _ = launch;
        Ok(())
    }
}

impl Resolver for Layers {
    fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
        let mut effective = Effective::resolve(layers, &Catalog)?;
        if self.switch.load(Ordering::SeqCst) && self.resolves.fetch_add(1, Ordering::SeqCst) > 0 {
            effective.provider = "beta".into();
            effective.model = "beta-model".into();
            effective.reasoning_effort = Some("high".into());
        }
        Ok(effective)
    }

    fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
        self.builds.fetch_add(1, Ordering::SeqCst);
        if effective.provider == "missing" {
            return Err("provider missing is not defined".into());
        }
        let (bytes, maintenance) = match effective.provider.as_str() {
            "alpha" => (200_000, 300_000),
            "beta" => (400_000, 600_000),
            "gamma" => (600_000, 900_000),
            _ => (80_000, 120_000),
        };
        Ok(Arc::new(Recorder {
            provider: effective.provider.clone(),
            model: effective.model.clone(),
            effort: effective.reasoning_effort.clone(),
            bytes,
            maintenance,
            calls: self.calls.clone(),
            gate: self.gate.subscribe(),
            block: self.block.clone(),
        }))
    }
}

pub(crate) struct Selection<'a> {
    pub(crate) provider: &'a str,
    pub(crate) model: &'a str,
    pub(crate) effort: &'a str,
}

pub(crate) fn patch(provider: &str, model: &str, effort: &str) -> Patch {
    Patch {
        provider: Some(Some(provider.into())),
        model: Some(Some(model.into())),
        reasoning_effort: Some(Some(effort.into())),
    }
}

pub(crate) async fn open() -> Fixture {
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
    let (gate, _) = watch::channel(false);
    let layers = Arc::new(Layers {
        calls: Arc::new(Mutex::new(Vec::new())),
        gate,
        switch: AtomicBool::new(false),
        builds: AtomicUsize::new(0),
        resolves: AtomicUsize::new(0),
        block: Arc::new(AtomicBool::new(true)),
    });
    let service = Service::provided(
        config.clone(),
        layers.clone(),
        Declaration::default(),
        Arc::new(Jobs),
    )
    .await
    .unwrap();
    let strand = service.weave().await.unwrap().strand;
    let store = Store::open(&config.database).await.unwrap();
    Fixture {
        _temp: temp,
        service,
        layers,
        store,
        strand,
    }
}

pub(crate) async fn declare(store: &Store, strand: &str, selection: Selection<'_>) {
    store
        .declare(
            Target::Strand,
            strand,
            &patch(selection.provider, selection.model, selection.effort),
            &santi_core::now(),
        )
        .await
        .unwrap();
}

pub(crate) async fn enqueue(store: &Store, strand: &str, text: &str) {
    let content = message::Content::text(text);
    store
        .accept_inbox(
            santi_core::InboxDraft {
                tag: &santi_core::tag("inbox"),
                strand,
                kind: message::Kind::Text,
                content: &content,
                source: None,
                created: &santi_core::now(),
            },
            500,
        )
        .await
        .unwrap();
}

pub(crate) async fn until(store: &Store, strand: &str, status: santi_core::turn::Status) {
    for _ in 0..200 {
        if store.latest(strand).await.unwrap().map(|turn| turn.status) == Some(status.clone()) {
            return;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!("turn did not reach {status:?}");
}

pub(crate) async fn observed(layers: &Layers, kind: &str, text: &str) -> Call {
    for _ in 0..200 {
        if let Some(call) = layers
            .calls
            .lock()
            .unwrap()
            .iter()
            .find(|call| call.kind == kind && call.text.contains(text))
            .cloned()
        {
            return call;
        }
        sleep(Duration::from_millis(10)).await;
    }
    panic!(
        "call {kind} containing {text:?} did not arrive: {:?}",
        layers.calls.lock().unwrap()
    );
}
