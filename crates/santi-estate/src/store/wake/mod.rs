use super::inbox::offer_in;
use super::{Store, read};
use keel::{Op, Rank, form};

mod support;
mod types;
use support::*;
pub use types::{WakeLease, WakeOfferDraft};

const ROUNDS: i64 = 3;

impl Store {
    pub async fn wake(&self, soul: &str) -> Result<Option<WakeLease>, String> {
        let Some(soul) = read::one(&self.core, "Soul", "tag", soul).await? else {
            return Ok(None);
        };
        let Some(row) = read::one(&self.core, "WakeLease", "soul", &soul.key().to_string()).await?
        else {
            return Ok(None);
        };
        decode(&self.core, &row).await.map(Some)
    }

    pub async fn scheduled_wakes(&self) -> Result<Vec<WakeLease>, String> {
        let rows = self
            .core
            .ask(
                &form("WakeLease")
                    .when("state", Op::Eq, "active")
                    .order("next_millis", Rank::Asc)
                    .order("id", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        let mut leases = Vec::with_capacity(rows.rows().len());
        for row in rows.rows() {
            leases.push(decode(&self.core, row).await?);
        }
        Ok(leases)
    }

    pub async fn enable_wake(
        &self,
        soul: &str,
        cadence_millis: i64,
        first_millis: i64,
        occurred: &str,
    ) -> Result<WakeLease, String> {
        if cadence_millis < 1 || first_millis < 1 {
            return Err("wake lease timing must be positive".to_string());
        }
        self.core
            .batch(async |tx| {
                let soul = read::need(tx, "Soul", "tag", soul).await?;
                let key = soul.to_string();
                let existing = tx
                    .one(&form("WakeLease").when("soul", Op::Eq, &key))
                    .await?;
                if existing
                    .as_ref()
                    .is_some_and(|row| row.text("state") == Some("active"))
                {
                    return Ok(());
                }
                let generation = existing
                    .as_ref()
                    .and_then(|row| row.int("generation"))
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or_else(|| adapt("wake lease generation is out of range"))?;
                let fields = [
                    ("generation", generation.to_string()),
                    ("state", "active".to_string()),
                    ("remaining", ROUNDS.to_string()),
                    ("cadence_millis", cadence_millis.to_string()),
                    ("next_millis", first_millis.to_string()),
                    ("updated", occurred.to_string()),
                ];
                let borrowed = fields
                    .iter()
                    .map(|(name, value)| (*name, value.as_str()))
                    .collect::<Vec<_>>();
                match existing {
                    Some(row) => tx.set("WakeLease", row.key(), &borrowed).await?,
                    None => {
                        let mut created = borrowed;
                        created.push(("created", occurred));
                        created.push(("soul", &key));
                        tx.put("WakeLease", &created).await?;
                    }
                }
                Ok(())
            })
            .await
            .map_err(read::error)?;
        self.need(soul).await
    }

    pub async fn disable_wake(
        &self,
        soul: &str,
        occurred: &str,
    ) -> Result<Option<WakeLease>, String> {
        self.core
            .batch(async |tx| {
                let soul = read::need(tx, "Soul", "tag", soul).await?;
                let Some(row) = tx
                    .one(&form("WakeLease").when("soul", Op::Eq, &soul.to_string()))
                    .await?
                else {
                    return Ok(());
                };
                if row.text("state") == Some("revoked") {
                    return Ok(());
                }
                let generation = advance(&row)?;
                let generation = generation.to_string();
                tx.set(
                    "WakeLease",
                    row.key(),
                    &[
                        ("generation", generation.as_str()),
                        ("state", "revoked"),
                        ("remaining", "0"),
                        ("updated", occurred),
                    ],
                )
                .await?;
                tx.unset("WakeLease", row.key(), &["next_millis"]).await?;
                Ok(())
            })
            .await
            .map_err(read::error)?;
        self.wake(soul).await
    }

    pub async fn renew_wake(
        &self,
        soul: &str,
        generation: u64,
        first_millis: i64,
        occurred: &str,
    ) -> Result<WakeLease, String> {
        self.core
            .batch(async |tx| {
                let row = lease(tx, soul).await?;
                expected(&row, generation)?;
                if row.text("state") == Some("revoked") {
                    return Err(adapt("wake lease is not permitted"));
                }
                let mut fields = vec![
                    ("state", "active"),
                    ("remaining", "3"),
                    ("updated", occurred),
                ];
                let next = first_millis.to_string();
                if row.text("state") != Some("active") || row.int("next_millis").is_none() {
                    fields.push(("next_millis", next.as_str()));
                }
                tx.set("WakeLease", row.key(), &fields).await?;
                Ok(())
            })
            .await
            .map_err(read::error)?;
        self.need(soul).await
    }

    pub async fn silence_wake(
        &self,
        soul: &str,
        generation: u64,
        occurred: &str,
    ) -> Result<WakeLease, String> {
        self.core
            .batch(async |tx| {
                let row = lease(tx, soul).await?;
                if row.text("state") == Some("silent")
                    && unsigned(&row, "generation")?
                        == generation
                            .checked_add(1)
                            .ok_or_else(|| adapt("wake lease generation is out of range"))?
                {
                    return Ok(());
                }
                expected(&row, generation)?;
                if matches!(row.text("state"), Some("revoked" | "silent")) {
                    return Ok(());
                }
                let generation = advance(&row)?;
                let generation = generation.to_string();
                tx.set(
                    "WakeLease",
                    row.key(),
                    &[
                        ("generation", generation.as_str()),
                        ("state", "silent"),
                        ("remaining", "0"),
                        ("updated", occurred),
                    ],
                )
                .await?;
                tx.unset("WakeLease", row.key(), &["next_millis"]).await?;
                Ok(())
            })
            .await
            .map_err(read::error)?;
        self.need(soul).await
    }

    pub async fn offer_wake(
        &self,
        draft: WakeOfferDraft<'_>,
    ) -> Result<Option<crate::store::Offer>, String> {
        if draft.notice.revision != draft.due_millis {
            return Err("wake notice revision must equal its scheduled time".to_string());
        }
        if draft.due_millis > draft.now_millis {
            return Err("wake notice cannot be offered before its scheduled time".to_string());
        }
        self.core
            .batch(async |tx| {
                let row = lease(tx, draft.soul).await?;
                if row.text("state") != Some("active")
                    || unsigned(&row, "generation")? != draft.generation
                    || row.int("next_millis") != Some(draft.due_millis)
                {
                    return Ok(None);
                }
                let remaining = row
                    .int("remaining")
                    .ok_or_else(|| adapt("wake lease remaining rounds missing"))?;
                if remaining < 1 {
                    return Err(adapt("active wake lease has no remaining rounds"));
                }
                let offer = offer_in(tx, draft.notice, draft.gate).await?;
                let remaining = remaining - 1;
                let last = draft.now_millis.to_string();
                let rounds = remaining.to_string();
                if remaining == 0 {
                    tx.set(
                        "WakeLease",
                        row.key(),
                        &[
                            ("state", "expired"),
                            ("remaining", rounds.as_str()),
                            ("last_wake_millis", last.as_str()),
                            ("updated", draft.occurred),
                        ],
                    )
                    .await?;
                    tx.unset("WakeLease", row.key(), &["next_millis"]).await?;
                } else {
                    let cadence = row
                        .int("cadence_millis")
                        .ok_or_else(|| adapt("wake lease cadence missing"))?;
                    let following = draft
                        .now_millis
                        .checked_add(cadence)
                        .ok_or_else(|| adapt("wake lease next time is out of range"))?
                        .to_string();
                    tx.set(
                        "WakeLease",
                        row.key(),
                        &[
                            ("remaining", rounds.as_str()),
                            ("next_millis", following.as_str()),
                            ("last_wake_millis", last.as_str()),
                            ("updated", draft.occurred),
                        ],
                    )
                    .await?;
                }
                Ok(Some(offer))
            })
            .await
            .map_err(read::error)
    }

    async fn need(&self, soul: &str) -> Result<WakeLease, String> {
        self.wake(soul)
            .await?
            .ok_or_else(|| "wake lease not found".to_string())
    }
}
