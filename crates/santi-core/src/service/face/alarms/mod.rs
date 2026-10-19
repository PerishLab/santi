use super::Service;
use crate::alarm;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

mod expression;
mod lifecycle;

pub trait Endpoint: Send + Sync {
    fn validate(&self, draft: &alarm::Draft) -> Result<(), String>;
    fn apply(&self, alarms: &[alarm::Alarm]) -> Result<alarm::Projection, String>;
}

pub(in crate::service) struct Lifecycle {
    endpoint: Arc<dyn Endpoint>,
    state: Mutex<State>,
}

pub(in crate::service) struct State {
    retry: Option<Instant>,
    projection: alarm::Projection,
}

pub(in crate::service) struct Unavailable;

impl Endpoint for Unavailable {
    fn validate(&self, _: &alarm::Draft) -> Result<(), String> {
        Err("alarm endpoint is unavailable on this runtime".into())
    }

    fn apply(&self, alarms: &[alarm::Alarm]) -> Result<alarm::Projection, String> {
        if !alarms.is_empty() {
            return Err("alarm endpoint is unavailable on this runtime".into());
        }
        Ok(alarm::Projection {
            endpoint: "unavailable".into(),
            timezone: "unknown".into(),
            applied: true,
            observed: None,
            error: None,
        })
    }
}

impl Lifecycle {
    pub(in crate::service) fn new(endpoint: Arc<dyn Endpoint>) -> Self {
        Self {
            endpoint,
            state: Mutex::new(State {
                retry: Some(Instant::now()),
                projection: alarm::Projection {
                    endpoint: "unknown".into(),
                    timezone: "unknown".into(),
                    applied: false,
                    observed: None,
                    error: None,
                },
            }),
        }
    }
}

impl Service {
    pub async fn alarmed(mut self, endpoint: Arc<dyn Endpoint>) -> Self {
        self.alarms = Arc::new(Lifecycle::new(endpoint));
        self.reconcile_alarms().await;
        self
    }

    pub async fn alarms(&self, soul: &str) -> Result<alarm::View, String> {
        let state = self.alarms.state.lock().await;
        let alarms = self
            .store
            .alarms()
            .await?
            .into_iter()
            .filter(|alarm| alarm.soul == soul)
            .collect();
        Ok(alarm::View {
            alarms,
            projection: state.projection.clone(),
        })
    }

    pub async fn set_alarm(&self, draft: alarm::Draft) -> Result<alarm::View, String> {
        expression::validate(&draft)?;
        self.alarms.endpoint.validate(&draft)?;
        let mut state = self.alarms.state.lock().await;
        let alarm = self.store.set_alarm(&draft).await?;
        self.apply_alarms(&mut state).await;
        Ok(alarm::View {
            alarms: vec![alarm],
            projection: state.projection.clone(),
        })
    }

    pub async fn end_alarm(&self, tag: &str, soul: &str) -> Result<alarm::View, String> {
        let mut state = self.alarms.state.lock().await;
        self.store.end_alarm(tag, soul).await?;
        self.apply_alarms(&mut state).await;
        Ok(alarm::View {
            alarms: Vec::new(),
            projection: state.projection.clone(),
        })
    }

    pub async fn fire_alarm(
        &self,
        tag: &str,
        occurrence: alarm::Occurrence,
    ) -> Result<crate::ingest::Receipt, String> {
        let accepted = self.store.accept_alarm(tag, &occurrence).await?;
        if accepted.inserted {
            self.inboxes.lock().unwrap().insert(
                accepted.receipt.strand.clone(),
                accepted.receipt.inbox.clone(),
            );
        }
        Ok(accepted.receipt)
    }
}
