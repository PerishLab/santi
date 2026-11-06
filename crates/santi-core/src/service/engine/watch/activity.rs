use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::watch;

use super::Service;
use crate::turn::activity::{Current, Phase, Snapshot};

pub(in crate::service) struct State {
    epoch: String,
    inner: Mutex<Registry>,
}

#[derive(Default)]
struct Registry {
    revision: u64,
    channels: HashMap<String, watch::Sender<Snapshot>>,
}

pub(in crate::service) struct Lease {
    state: Arc<State>,
    strand: String,
    turn: String,
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.state.end(&self.strand, &self.turn);
    }
}

impl State {
    pub(in crate::service) fn new() -> Self {
        Self {
            epoch: crate::tag("activity"),
            inner: Mutex::new(Registry::default()),
        }
    }

    pub(in crate::service) fn lease(self: &Arc<Self>, strand: &str, turn: &str) -> Lease {
        Lease {
            state: self.clone(),
            strand: strand.into(),
            turn: turn.into(),
        }
    }

    fn channel<'a>(&self, registry: &'a mut Registry, strand: &str) -> &'a watch::Sender<Snapshot> {
        registry
            .channels
            .entry(strand.to_string())
            .or_insert_with(|| {
                watch::Sender::new(Snapshot {
                    epoch: self.epoch.clone(),
                    revision: registry.revision,
                    strand: strand.to_string(),
                    current: None,
                })
            })
    }

    pub(in crate::service) fn subscribe(&self, strand: &str) -> watch::Receiver<Snapshot> {
        let mut registry = self.inner.lock().unwrap();
        registry.channels.retain(|_, channel| {
            channel.receiver_count() > 0 || channel.borrow().current.is_some()
        });
        self.channel(&mut registry, strand).subscribe()
    }

    pub(in crate::service) fn start(&self, strand: &str, turn: &str) {
        let mut registry = self.inner.lock().unwrap();
        self.channel(&mut registry, strand);
        registry.replace(
            strand,
            Some(Current {
                turn: turn.to_string(),
                round: 0,
                phase: Phase::Preparing,
                call: None,
            }),
        );
    }

    pub(in crate::service) fn update(&self, strand: &str, next: Current) {
        let mut registry = self.inner.lock().unwrap();
        let Some(channel) = registry.channels.get(strand) else {
            return;
        };
        let accepted = channel.borrow().current.as_ref().is_some_and(|current| {
            current.turn == next.turn && current.round <= next.round && current != &next
        });
        if accepted {
            registry.replace(strand, Some(next));
        }
    }

    pub(in crate::service) fn end(&self, strand: &str, turn: &str) {
        let mut registry = self.inner.lock().unwrap();
        let Some(channel) = registry.channels.get(strand) else {
            return;
        };
        let accepted = channel
            .borrow()
            .current
            .as_ref()
            .is_some_and(|current| current.turn == turn);
        if accepted {
            registry.replace(strand, None);
        }
        if registry.channels.get(strand).is_some_and(|channel| {
            channel.receiver_count() == 0 && channel.borrow().current.is_none()
        }) {
            registry.channels.remove(strand);
        }
    }
}

impl Registry {
    fn replace(&mut self, strand: &str, current: Option<Current>) {
        let channel = self.channels.get(strand).expect("registered activity");
        self.revision = self
            .revision
            .checked_add(1)
            .expect("activity revision exhausted");
        let previous = channel.borrow().clone();
        channel.send_replace(Snapshot {
            revision: self.revision,
            current,
            ..previous
        });
    }
}

impl Service {
    pub async fn observe(&self, strand: &str) -> Result<watch::Receiver<Snapshot>, String> {
        if self.store.strand(strand).await?.is_none() {
            return Err("strand not found".into());
        }
        Ok(self.activity.subscribe(strand))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current(turn: &str, round: usize) -> Current {
        Current {
            turn: turn.into(),
            round,
            phase: Phase::Requesting,
            call: None,
        }
    }

    #[tokio::test]
    async fn retained() {
        let state = State::new();
        state.start("strand", "turn");
        state.update("strand", current("turn", 1));
        let mut receiver = state.subscribe("strand");
        let initial = receiver.borrow_and_update().clone();
        assert_eq!(initial.current, Some(current("turn", 1)));
        state.update("strand", current("turn", 2));
        state.update("strand", current("turn", 3));
        receiver.changed().await.unwrap();
        let latest = receiver.borrow_and_update().clone();
        assert_eq!(latest.current, Some(current("turn", 3)));
        assert!(latest.revision > initial.revision);
        state.update("strand", current("turn", 3));
        assert!(!receiver.has_changed().unwrap());
        state.end("strand", "turn");
        receiver.changed().await.unwrap();
        assert!(receiver.borrow_and_update().current.is_none());
        state.update("strand", current("turn", 4));
        assert!(!receiver.has_changed().unwrap());
    }

    #[tokio::test]
    async fn cancelled() {
        let state = Arc::new(State::new());
        state.start("strand", "turn");
        let receiver = state.subscribe("strand");
        let lease = state.lease("strand", "turn");
        let task = tokio::spawn(async move {
            let _lease = lease;
            std::future::pending::<()>().await;
        });
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(receiver.borrow().current.is_none());
    }

    #[test]
    fn stale() {
        let state = State::new();
        state.start("strand", "old");
        state.update("strand", current("old", 2));
        let receiver = state.subscribe("strand");
        let prior = receiver.borrow().clone();
        state.update("strand", current("old", 1));
        assert_eq!(*receiver.borrow(), prior);
        state.start("strand", "new");
        let latest = receiver.borrow().clone();
        state.end("strand", "old");
        state.update("strand", current("old", 3));
        assert_eq!(*receiver.borrow(), latest);
        state.start("other", "foreign");
        assert_eq!(*receiver.borrow(), latest);
        assert_ne!(state.subscribe("other").borrow().strand, latest.strand);
    }

    #[test]
    fn release() {
        let state = State::new();
        let receiver = state.subscribe("strand");
        let epoch = receiver.borrow().epoch.clone();
        state.start("strand", "turn");
        state.end("strand", "turn");
        let revision = receiver.borrow().revision;
        drop(receiver);
        let _other = state.subscribe("other");
        assert!(!state.inner.lock().unwrap().channels.contains_key("strand"));
        let receiver = state.subscribe("strand");
        assert_eq!(receiver.borrow().revision, revision);
        assert_eq!(receiver.borrow().epoch, epoch);
        let fresh = State::new();
        assert_ne!(fresh.subscribe("strand").borrow().epoch, epoch);
        assert!(fresh.subscribe("strand").borrow().current.is_none());
        drop(state);
        assert!(receiver.has_changed().is_err());
    }

    #[test]
    fn race() {
        for _ in 0..64 {
            let state = std::sync::Arc::new(State::new());
            state.start("strand", "turn");
            let writer = state.clone();
            let task = std::thread::spawn(move || writer.update("strand", current("turn", 1)));
            let mut receiver = state.subscribe("strand");
            let initial = receiver.borrow_and_update().clone();
            task.join().unwrap();
            assert!(initial.current == Some(current("turn", 1)) || receiver.has_changed().unwrap());
        }
    }
}
