use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};

use super::{Service, address::Address};
use crate::{environment, message, stream};
use band::{Band, banded, refused};
use remind::reminded;

mod band;
mod remind;
mod unresolved;

pub(in crate::service) struct Observation<'a> {
    pub(in crate::service) address: Address<&'a str>,
    pub(in crate::service) round: usize,
    pub(in crate::service) provider: &'a str,
    pub(in crate::service) model: &'a str,
}

const NOTICES: usize = 128;
pub(in crate::service) const REFERENCE: usize = 96 * 1024;

#[derive(Debug, Clone)]
pub(in crate::service) enum Event {
    Observed(Observed),
    Environment(Environment),
}

impl Event {
    fn turn(&self) -> &str {
        match self {
            Self::Observed(event) => &event.address.turn,
            Self::Environment(event) => &event.address.turn,
        }
    }

    fn dedupe(&self) -> Option<String> {
        match self {
            Self::Observed(_) => None,
            Self::Environment(event) => Some(event.issue.dedupe(&event.address.strand)),
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::service) struct Environment {
    address: Address<String>,
    issue: environment::Unresolved,
}

#[derive(Debug, Clone)]
pub(in crate::service) struct Observed {
    pub(in crate::service) address: Address<String>,
    pub(in crate::service) round: usize,
    pub(in crate::service) provider: String,
    pub(in crate::service) model: String,
}

#[derive(Clone, Debug)]
pub(in crate::service) struct Bus {
    inner: Arc<Mutex<State>>,
}

#[derive(Debug)]
struct State {
    queue: VecDeque<Event>,
    held: HashSet<String>,
    bands: HashMap<String, Band>,
    capacity: usize,
}

impl Bus {
    pub(in crate::service) fn new() -> Self {
        Self::sized(NOTICES)
    }

    fn sized(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(State {
                queue: VecDeque::new(),
                held: HashSet::new(),
                bands: HashMap::new(),
                capacity,
            })),
        }
    }

    pub(in crate::service) fn publish(&self, event: Event) -> bool {
        let dedupe = event.dedupe();
        let mut state = self.inner.lock().unwrap();
        if let Some(key) = dedupe.as_ref()
            && state.held.contains(key)
        {
            return false;
        }
        if let Event::Observed(observed) = &event
            && state.queue.iter().any(|queued| {
                matches!(queued, Event::Observed(held) if held.address.turn == observed.address.turn)
            })
        {
            return false;
        }
        if state.queue.len() >= state.capacity {
            return false;
        }
        if let Some(key) = dedupe {
            state.held.insert(key);
        }
        state.queue.push_back(event);
        true
    }

    fn reached(&self, strand: &str, band: Band) -> bool {
        let mut state = self.inner.lock().unwrap();
        let held = state.bands.get(strand).copied().unwrap_or(Band::Under);
        if band == Band::Under {
            state.bands.remove(strand);
        } else {
            state.bands.insert(strand.to_string(), band);
        }
        band > held
    }

    fn forget(&self, strand: &str) {
        self.inner.lock().unwrap().bands.remove(strand);
    }

    pub(in crate::service) fn drained(&self, turn: &str) -> Vec<Event> {
        let mut state = self.inner.lock().unwrap();
        let mut drained = Vec::new();
        let mut kept = VecDeque::with_capacity(state.queue.len());
        while let Some(event) = state.queue.pop_front() {
            if event.turn() == turn {
                drained.push(event);
            } else {
                kept.push_back(event);
            }
        }
        state.queue = kept;
        drained
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Service {
    pub(in crate::service) fn observed(&self, observation: Observation<'_>) {
        let _ = self.notices.publish(Event::Observed(Observed {
            address: observation.address.owned(),
            round: observation.round,
            provider: observation.provider.to_string(),
            model: observation.model.to_string(),
        }));
    }

    pub(in crate::service) fn rearm(&self, strand: &str) {
        self.notices.forget(strand);
    }

    pub(in crate::service) fn unresolved(
        &self,
        address: Address<&str>,
        issue: environment::Unresolved,
    ) {
        let _ = self.notices.publish(Event::Environment(Environment {
            address: address.owned(),
            issue,
        }));
    }

    pub(in crate::service) async fn noticed(&self, turn: &str) {
        for event in self.notices.drained(turn) {
            if let Err(error) = self.absorbed(event).await {
                eprintln!("santi: internal runtime notice failed: {error}");
            }
        }
    }

    async fn absorbed(&self, event: Event) -> Result<(), String> {
        match event {
            Event::Observed(event) => self.remind(event).await,
            Event::Environment(event) => unresolved::warn(self, event).await,
        }
    }

    async fn remind(&self, event: Observed) -> Result<(), String> {
        if self.settlement(&event.address.turn).is_some() {
            return Ok(());
        }
        let gauge = self.gauge(&event.address.strand, false).await?;
        if refused(&gauge) {
            return Ok(());
        }
        let band = banded(&gauge);
        if !self.notices.reached(&event.address.strand, band) {
            return Ok(());
        }
        let content = reminded(&event, &gauge, band);
        let message = self
            .store
            .place(santi_estate::MessageDraft {
                tag: &crate::tag("msg"),
                strand: &event.address.strand,
                actor: message::Role::System,
                actor_id: crate::SYSTEM,
                kind: message::Kind::SantiSystem,
                content: &content,
                state: message::State::Fixed,
                request: false,
                created: &crate::now(),
            })
            .await?;
        self.publish(
            &event.address.strand,
            stream::Payload::Message(crate::message::Beat::Created { message }),
        );
        Ok(())
    }
}
