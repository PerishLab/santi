pub use face::alarms;
mod engine;
use engine::{address, drive, error, notice, render, thinking, timing, watch::activity};
mod face;
pub(crate) use engine::provider::Fixed;
pub use engine::provider::Resolver;
pub use face::Admission;
mod flow;
mod jobs;
pub use jobs::{
    Draft as JobDraft, Launch as JobLaunch, Observation as JobObservation, Read as JobRead,
    Supervisor as JobSupervisor, Terminal as JobTerminal,
};
pub mod interrupt;
mod text;
mod tools;
mod traces;

use crate::inference::{Declaration, Effective, Origin};
use santi_provider::Provider;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::broadcast;

use crate::{Ruled, Transition};
use crate::{budget, stream};
use santi_estate::Store;

pub const MEMORY: usize = 32_768;

pub const RETENTION: u64 = 7 * 24 * 60 * 60;

#[derive(Clone)]
pub struct Service {
    pub(crate) store: Store,
    pub(crate) context: radicula_context::Context,
    providers: Arc<dyn crate::provider::Resolver>,
    declaration: Declaration,
    pins: Arc<Mutex<HashMap<String, Arc<dyn Provider>>>>,
    pub(crate) config: Config,
    renders: Arc<Mutex<HashMap<render::Key, crate::render::View>>>,
    library: Arc<crate::library::Catalog>,
    binding: radicula_lock::Gate,
    streams: broadcast::Sender<stream::Event>,
    activity: Arc<activity::State>,
    errors: broadcast::Sender<Transition>,
    notices: notice::Bus,
    inboxes: Arc<Mutex<HashMap<String, String>>>,
    budgets: Arc<Mutex<HashMap<String, budget::Execution>>>,
    limit: Option<budget::Execution>,
    memory_bytes: usize,
    identities: Arc<BTreeMap<String, crate::soul::Identity>>,
    settlements: Arc<Mutex<HashMap<String, Settlement>>>,
    pressure: radicula_lock::Gate,
    closing: Arc<AtomicBool>,
    controls: Arc<Mutex<HashMap<String, interrupt::Control>>>,
    refusals: Arc<Mutex<HashMap<String, usize>>>,
    deadline: Arc<Mutex<Option<Instant>>>,
    degraded: Arc<AtomicBool>,
    supervisor: Arc<dyn jobs::Supervisor>,
    handoffs: Arc<Mutex<HashMap<String, usize>>>,
    retention: Duration,
    alarms: Arc<alarms::Lifecycle>,
    pub(crate) capability: Option<Arc<crate::capability::Issuer>>,
}

struct Settlement {
    reason: String,
    detail: String,
    replies: usize,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database: String,
    pub runtime: String,
    pub execution: String,
    pub bind: Option<String>,
    pub constitution: Option<String>,
    pub environment: BTreeMap<String, String>,
}

pub struct Delivery<'a> {
    pub subscription: &'a str,
    pub id: &'a str,
    pub digest: &'a str,
}

pub struct Envelope<'a> {
    pub soul: &'a str,
    pub label: &'a str,
    pub text: String,
    pub source: Option<crate::ingest::Source>,
}

impl Service {
    pub async fn open(config: Config, provider: Arc<dyn Provider>) -> Result<Self, String> {
        Self::supervised(config, provider, Arc::new(jobs::Unavailable)).await
    }

    pub async fn supervised(
        config: Config,
        provider: Arc<dyn Provider>,
        supervisor: Arc<dyn jobs::Supervisor>,
    ) -> Result<Self, String> {
        Self::provided(
            config,
            Arc::new(crate::provider::Fixed::new(provider)),
            Declaration::default(),
            supervisor,
        )
        .await
    }

    pub async fn provided(
        config: Config,
        providers: Arc<dyn crate::provider::Resolver>,
        declaration: Declaration,
        supervisor: Arc<dyn jobs::Supervisor>,
    ) -> Result<Self, String> {
        crate::environment::validate(&config.environment)?;
        let store = Store::open(&config.database).await?;
        store.seed(crate::GENESIS, &crate::now()).await?;
        let traces = traces::Writer::start(store.clone());
        let sink = radicula_trace::Sink::from(traces);
        let context = radicula_context::Context::root().with(sink);
        store
            .recover_turns("santi.cold_start", &crate::now())
            .await?;
        store.interrupted(&crate::now()).await?;
        store.interrupt().await?;
        let degraded = store
            .active_incident_count(crate::drive::Error::Failed.descriptor().code)
            .await?
            > 0;
        let domain = radicula_lock::Factory::memory();
        let binding = domain
            .gate("binding", radicula_lock::Policy::EXCLUSIVE)
            .map_err(|error| error.to_string())?;
        let pressure = domain
            .gate("pressure", radicula_lock::Policy::EXCLUSIVE)
            .map_err(|error| error.to_string())?;
        let service = Self {
            store,
            context,
            providers,
            declaration,
            pins: Arc::new(Mutex::new(HashMap::new())),
            config,
            renders: Arc::new(Mutex::new(HashMap::new())),
            library: Arc::new(crate::library::Catalog::default()),
            binding,
            streams: broadcast::channel(1024).0,
            activity: Arc::new(activity::State::new()),
            errors: broadcast::channel(1024).0,
            notices: notice::Bus::new(),
            inboxes: Arc::new(Mutex::new(HashMap::new())),
            budgets: Arc::new(Mutex::new(HashMap::new())),
            limit: None,
            memory_bytes: MEMORY,
            identities: Arc::new(BTreeMap::new()),
            settlements: Arc::new(Mutex::new(HashMap::new())),
            pressure,
            closing: Arc::new(AtomicBool::new(false)),
            controls: Arc::new(Mutex::new(HashMap::new())),
            refusals: Arc::new(Mutex::new(HashMap::new())),
            deadline: Arc::new(Mutex::new(None)),
            degraded: Arc::new(AtomicBool::new(degraded)),
            supervisor,
            handoffs: Arc::new(Mutex::new(HashMap::new())),
            retention: Duration::from_secs(RETENTION),
            alarms: Arc::new(alarms::Lifecycle::new(Arc::new(alarms::Unavailable))),
            capability: None,
        };
        if let Err(error) = service.tidy().await {
            eprintln!("santi: start sweep failed: {error}");
        }
        Ok(service)
    }

    pub fn authorized(mut self, issuer: Option<crate::capability::Issuer>) -> Self {
        self.capability = issuer.map(Arc::new);
        self
    }

    pub fn bounded(mut self, limit: budget::Execution) -> Result<Self, String> {
        limit.validate()?;
        self.limit = Some(limit);
        Ok(self)
    }

    pub fn memory(mut self, bytes: usize) -> Result<Self, String> {
        if bytes == 0 {
            return Err("shared memory bytes must be greater than zero".into());
        }
        self.memory_bytes = bytes;
        Ok(self)
    }

    pub async fn ration(&self, strand: &str, budget: budget::Execution) -> Result<(), String> {
        budget.validate()?;
        if self.store.strand(strand).await?.is_none() {
            return Err("strand not found".to_string());
        }
        self.budgets
            .lock()
            .unwrap()
            .insert(strand.to_string(), budget);
        Ok(())
    }

    pub(in crate::service) fn rationed(&self, strand: &str) -> Option<budget::Execution> {
        self.budgets
            .lock()
            .unwrap()
            .get(strand)
            .cloned()
            .or_else(|| self.limit.clone())
    }

    pub fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
    }

    pub fn closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }
}

impl Service {
    pub(crate) async fn selected(&self, strand: &str) -> Result<Effective, String> {
        let strand = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let soul = self
            .store
            .soul(&strand.soul)
            .await?
            .ok_or_else(|| "soul not found".to_string())?;
        let layers = [
            (&self.declaration, Origin::Service),
            (&soul.inference, Origin::Soul),
            (&strand.inference, Origin::Strand),
        ];
        self.providers.resolve(&layers)
    }

    pub(crate) async fn preview(&self, strand: &str) -> Result<Arc<dyn Provider>, String> {
        let effective = self.selected(strand).await?;
        self.providers.build(&effective)
    }

    pub(crate) async fn current(&self, strand: &str) -> Result<Arc<dyn Provider>, String> {
        if let Some(turn) = self.store.latest(strand).await?
            && matches!(turn.status, crate::turn::Status::Running)
        {
            return self.recovery(&turn.id).await;
        }
        self.preview(strand).await
    }

    pub(crate) fn remember(&self, turn: &str, provider: Arc<dyn Provider>) {
        self.pins.lock().unwrap().insert(turn.to_string(), provider);
    }

    pub(crate) fn forget(&self, turn: &str) {
        self.pins.lock().unwrap().remove(turn);
    }

    pub(crate) fn pinned(&self, turn: &str) -> Result<Arc<dyn Provider>, String> {
        self.pins.lock().unwrap().get(turn).cloned().ok_or_else(|| {
            format!("turn {turn} has no pinned provider; inference history is unknown")
        })
    }

    pub(crate) async fn recovery(&self, turn: &str) -> Result<Arc<dyn Provider>, String> {
        if let Ok(provider) = self.pinned(turn) {
            return Ok(provider);
        }
        let Some(snapshot) = self.store.inference(turn).await? else {
            return Err(format!(
                "turn {turn} inference snapshot missing: history unknown"
            ));
        };
        let provider = self.providers.build(&snapshot)?;
        self.remember(turn, provider.clone());
        Ok(provider)
    }
}
