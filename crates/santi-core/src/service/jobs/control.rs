use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

use super::handoff::Handoff;
use super::{observe, terminal, *};
use santi_estate::{JobRecord, Prepared};

impl Service {
    pub(crate) async fn permit(
        &self,
        strand: &str,
        turn: &str,
        call: &str,
        effect: &str,
    ) -> Result<String, String> {
        let soul = self
            .store
            .strand(strand)
            .await?
            .map(|strand| strand.soul)
            .ok_or_else(|| "strand not found".to_string())?;
        let capability = crate::tag("jobcap");
        let digest = hex::encode(Sha256::digest(capability.as_bytes()));
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_millis();
        let expires = i64::try_from(now.saturating_add(120_000))
            .map_err(|_| "job capability expiry is out of range".to_string())?;
        self.store
            .create_capability(santi_estate::CapabilityDraft {
                digest: &digest,
                expires,
                soul: &soul,
                strand,
                turn,
                call,
                effect,
                created: &crate::now(),
            })
            .await?;
        Ok(capability)
    }

    pub async fn spawn(&self, capability: &str, draft: Draft) -> Result<job::Accepted, String> {
        let normalized = draft::normalize(draft)?;
        let capability = hex::encode(Sha256::digest(capability.as_bytes()));
        let (origin, consumed) = self
            .store
            .capability(&capability, observe::epoch()?)
            .await?;
        let environment = if consumed {
            None
        } else {
            Some(santi_estate::Environment(
                self.resolved_environment(&job::Origin {
                    soul: origin.soul.as_str(),
                    strand: origin.strand.as_str(),
                    turn: origin.turn.as_str(),
                    call: origin.call.as_str(),
                    effect: origin.effect.as_str(),
                })
                .await?,
            ))
        };
        let tag = crate::tag("job");
        let generation = crate::tag("stamp");
        let supervisor = crate::tag("sidecar");
        let now = crate::now();
        let preparing = Handoff::new(&self.handoffs, &generation);
        let prepared = self
            .store
            .prepare_job(
                &capability,
                santi_estate::JobDraft {
                    environment: environment.as_ref(),
                    tag: &tag,
                    description: &normalized.description,
                    command: &normalized.command,
                    cwd: normalized.cwd.as_deref(),
                    timeout_seconds: normalized.timeout,
                    output_limit_bytes: normalized.output,
                    remind_every_seconds: normalized.remind,
                    notify_on_terminal: normalized.notify.is_some(),
                    request_sha256: &normalized.digest,
                    generation: &generation,
                    supervisor_ref: &supervisor,
                    created: &now,
                },
                observe::epoch()?,
            )
            .await?;
        let record = match prepared {
            Prepared::New(record) | Prepared::Existing(record) => record,
        };
        let _handoff = Handoff::new(&self.handoffs, &record.generation);
        drop(preparing);
        if record.job.state == job::State::Submitting {
            self.detach(&record)?;
        }
        let record = if record.job.state == job::State::Submitting {
            self.store.accept_job(&record.job.id, &crate::now()).await?
        } else {
            record
        };
        Ok(job::Accepted { job: record.job })
    }

    pub async fn job(&self, soul: &str, id: &str) -> Result<Option<job::Job>, String> {
        let Some(record) = self.store.job_record(id).await? else {
            return Ok(None);
        };
        if record.job.origin.soul != soul {
            return Ok(None);
        }
        if record.job.state.terminal() || record.job.acknowledged.is_some() {
            return Ok(Some(record.job));
        }
        self.refresh(record).await.map(|record| Some(record.job))
    }

    pub async fn jobs(&self, soul: &str) -> Result<Vec<job::Job>, String> {
        let active = self.store.active_jobs().await?;
        for record in active
            .into_iter()
            .filter(|record| record.job.origin.soul == soul)
        {
            if let Err(error) = self.refresh(record.clone()).await {
                eprintln!(
                    "santi: job observation failed job={} detail={error}",
                    record.job.id
                );
            }
        }
        self.store.jobs(soul).await
    }

    pub async fn cancel(&self, soul: &str, id: &str) -> Result<Option<job::Job>, String> {
        let Some(record) = self.store.job_record(id).await? else {
            return Ok(None);
        };
        if record.job.origin.soul != soul {
            return Ok(None);
        }
        if record.job.state.terminal() {
            terminal::report(self, &record).await;
            return Ok(Some(record.job));
        }
        let record = self.refresh(record).await?;
        if record.job.state.terminal() {
            terminal::report(self, &record).await;
            return Ok(Some(record.job));
        }
        let record = self
            .store
            .transition_job(
                id,
                santi_estate::TransitionDraft {
                    reason: Some("cancel_requested"),
                    ..santi_estate::TransitionDraft::new(job::State::Cancelling, &crate::now())
                },
            )
            .await?;
        self.supervisor.stop(&self.launch(&record)?)?;
        let record = self.refresh(record).await?;
        terminal::report(self, &record).await;
        Ok(Some(record.job))
    }

    pub async fn ack(&self, soul: &str, id: &str) -> Result<Option<job::Job>, String> {
        let Some(record) = self.store.job_record(id).await? else {
            return Ok(None);
        };
        if record.job.origin.soul != soul {
            return Ok(None);
        }
        if record.job.acknowledged.is_some() {
            return Ok(Some(record.job));
        }
        let mut record = if record.job.state.terminal() {
            record
        } else {
            self.refresh(record).await?
        };
        if !record.job.state.terminal() {
            return Err("only a terminal job can be acknowledged".to_string());
        }
        if record.notify_on_terminal && record.terminal_notified.is_none() {
            if let Err(error) = terminal::finish(self, record.clone()).await {
                return Err(format!(
                    "terminal job notification is not yet durably recorded; retry acknowledgement: {error}"
                ));
            }
            record = self
                .store
                .job_record(id)
                .await?
                .ok_or_else(|| "job not found".to_string())?;
            if record.terminal_notified.is_none() {
                return Err(
                    "terminal job notification is not yet durably recorded; retry acknowledgement"
                        .to_string(),
                );
            }
        }
        self.supervisor.acknowledge(&self.launch(&record)?)?;
        self.store
            .acknowledge_job(id, &crate::now())
            .await
            .map(|record| Some(record.job))
    }

    fn detach(&self, record: &JobRecord) -> Result<(), String> {
        self.launch(record)
            .and_then(|launch| self.supervisor.detach(&launch))
    }
}
