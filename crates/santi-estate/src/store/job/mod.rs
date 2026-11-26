use super::{Store, read};
use keel::{Op, form};
use santi_model::job;

mod attention;
mod projection;
mod query;
mod types;
mod write;

pub use types::{Attention, Capability, Environment, Expired, Job, Prepared, Record, Transition};

impl Store {
    pub async fn grant(&self, draft: Capability<'_>) -> Result<(), String> {
        self.core
            .batch(async |tx| write::capability(tx, draft).await)
            .await
            .map_err(read::error)
    }

    pub async fn capability(&self, digest: &str, now: i64) -> Result<(job::Origin, bool), String> {
        let row = read::one(&self.core, "JobCapability", "digest", digest)
            .await?
            .ok_or_else(|| "invalid job capability".to_string())?;
        if row.int("consumed").is_none() && now > read::int(&row, "expires")? {
            return Err("job capability expired".to_string());
        }
        Ok((
            job::Origin {
                soul: read::related(&self.core, "Soul", read::int(&row, "soul")?).await?,
                strand: read::related(&self.core, "Strand", read::int(&row, "strand")?).await?,
                turn: read::related(&self.core, "Turn", read::int(&row, "turn")?).await?,
                call: read::related(&self.core, "ToolCall", read::int(&row, "call")?).await?,
                effect: read::related(&self.core, "StrandEffect", read::int(&row, "effect")?)
                    .await?,
            },
            row.int("consumed").is_some(),
        ))
    }

    pub async fn sweep(&self, now: i64) -> Result<usize, String> {
        self.core
            .batch(async |tx| {
                let rows = tx
                    .ask(
                        &form("JobCapability")
                            .when("expires", Op::Lt, &now.to_string())
                            .missing("consumed"),
                    )
                    .await?;
                for row in rows.rows() {
                    tx.end("JobCapability", row.key()).await?;
                }
                Ok(rows.rows().len())
            })
            .await
            .map_err(read::error)
    }

    pub async fn submit(&self, digest: &str, draft: Job<'_>, now: i64) -> Result<Prepared, String> {
        let (created, tag) = self
            .core
            .batch(async |tx| write::prepare(tx, digest, draft, now).await)
            .await
            .map_err(read::error)?;
        let record = projection::Projection::new(self)
            .record(&tag)
            .await?
            .ok_or_else(|| "prepared job missing".to_string())?;
        if created {
            Ok(Prepared::New(record))
        } else {
            Ok(Prepared::Existing(record))
        }
    }

    pub async fn claim(&self, tag: &str, occurred: &str) -> Result<Record, String> {
        self.core
            .batch(async |tx| write::accept(tx, tag, occurred).await)
            .await
            .map_err(read::error)?;
        need(self, tag).await
    }

    pub async fn transit(&self, tag: &str, draft: Transition<'_>) -> Result<Record, String> {
        self.core
            .batch(async |tx| write::Writer::new(tx).transition(tag, draft).await)
            .await
            .map_err(read::error)?;
        need(self, tag).await
    }

    pub async fn ack(&self, tag: &str, occurred: &str) -> Result<Record, String> {
        self.core
            .batch(async |tx| write::Writer::new(tx).acknowledge(tag, occurred).await)
            .await
            .map_err(read::error)?;
        need(self, tag).await
    }

    pub async fn job(&self, soul: &str, tag: &str) -> Result<Option<job::Job>, String> {
        Ok(projection::Projection::new(self)
            .record(tag)
            .await?
            .filter(|record| record.job.origin.soul == soul)
            .map(|record| record.job))
    }

    pub async fn tracked(&self, tag: &str) -> Result<Option<Record>, String> {
        projection::Projection::new(self).record(tag).await
    }

    pub async fn jobs(&self, soul: &str) -> Result<Vec<job::Job>, String> {
        Ok(projection::Projection::new(self)
            .jobs(soul)
            .await?
            .into_iter()
            .map(|record| record.job)
            .collect())
    }

    pub async fn live(&self) -> Result<Vec<Record>, String> {
        projection::Projection::new(self).active().await
    }

    pub async fn unnotified(&self, limit: usize) -> Result<Vec<Record>, String> {
        projection::Projection::new(self).unnotified(limit).await
    }

    pub async fn stale(&self, cutoff: &str, limit: usize) -> Result<Vec<Expired>, String> {
        projection::Projection::new(self)
            .expired(cutoff, limit)
            .await
    }

    pub async fn retained(&self, key: &str) -> Result<bool, String> {
        if read::one(&self.core, "Job", "tag", key).await?.is_some() {
            return Ok(true);
        }
        Ok(read::one(&self.core, "Job", "generation", key)
            .await?
            .is_some())
    }

    pub async fn purge(&self, tag: &str, cutoff: &str) -> Result<bool, String> {
        self.core
            .batch(async |tx| {
                let Some(job) = tx.one(&form("Job").when("tag", Op::Eq, tag)).await? else {
                    return Ok(false);
                };
                if !terminal(job.text("state"))
                    || job
                        .text("acknowledged")
                        .is_none_or(|acknowledged| acknowledged > cutoff)
                {
                    return Ok(false);
                }
                let capabilities = tx
                    .ask(&form("JobCapability").when("consumed", Op::Eq, &job.key().to_string()))
                    .await?;
                for capability in capabilities.rows() {
                    tx.end("JobCapability", capability.key()).await?;
                }
                tx.end("Job", job.key()).await?;
                Ok(true)
            })
            .await
            .map_err(read::error)
    }
}

async fn need(store: &Store, tag: &str) -> Result<Record, String> {
    projection::Projection::new(store)
        .record(tag)
        .await?
        .ok_or_else(|| "job not found".to_string())
}

fn terminal(state: Option<&str>) -> bool {
    matches!(
        state,
        Some("succeeded" | "failed" | "timed_out" | "cancelled" | "unknown")
    )
}
