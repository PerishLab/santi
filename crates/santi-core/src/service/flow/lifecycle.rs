use crate::service::{Service, drive};
use crate::{Transition, stream};
use santi_error::Ruled;
use std::sync::atomic::Ordering;
use tokio::sync::broadcast;

impl Service {
    pub async fn resume(&self) -> Result<(), String> {
        self.dispatched().await;
        let pending = self.store.pending_strands().await?;
        for strand in pending {
            let outcome = self
                .poke(&strand, "strand_send", None, "cold_start_resume")
                .await;
            if let drive::Outcome::Failed(error) = outcome
                && error.code == crate::catalog::UNSAVED.code
            {
                return Err(format!(
                    "cold-start recovery could not persist driver truth for strand {strand}: {}",
                    error.message
                ));
            }
        }
        Ok(())
    }

    pub fn degraded(&self) -> bool {
        self.degraded.load(Ordering::SeqCst)
    }

    pub async fn strained(&self) -> usize {
        match self
            .store
            .active_incident_count(crate::drive::Error::Failed.descriptor().code)
            .await
        {
            Ok(count) => count,
            Err(error) => {
                self.degraded.store(true, Ordering::SeqCst);
                eprintln!("santi: drive health count failed: {error}");
                0
            }
        }
    }

    pub(in crate::service) fn degrade(&self) {
        self.degraded.store(true, Ordering::SeqCst);
    }

    pub(in crate::service) async fn refreshed(&self) {
        match self
            .store
            .active_incident_count(crate::drive::Error::Failed.descriptor().code)
            .await
        {
            Ok(count) => self.degraded.store(count > 0, Ordering::SeqCst),
            Err(error) => {
                self.degraded.store(true, Ordering::SeqCst);
                eprintln!("santi: drive health refresh failed: {error}");
            }
        }
    }

    pub async fn listen(&self) -> broadcast::Receiver<stream::Event> {
        let receiver = self.streams.subscribe();
        self.dispatched().await;
        receiver
    }

    pub async fn harken(&self) -> broadcast::Receiver<Transition> {
        let receiver = self.errors.subscribe();
        self.dispatched().await;
        receiver
    }
}
