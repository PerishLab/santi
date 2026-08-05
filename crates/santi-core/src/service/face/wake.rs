use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::Service;
use crate::wake;

impl Service {
    pub async fn wake(&self, soul: &str) -> Result<Option<wake::Lease>, String> {
        if self.store.soul(soul).await?.is_none() {
            return Ok(None);
        }
        let cadence = millis(self.clock.cadence())?;
        match self.store.wake(soul).await? {
            Some(record) => project(record).map(Some),
            None => Ok(Some(wake::Lease::revoked(soul, cadence))),
        }
    }

    pub async fn enable_wake(&self, soul: &str) -> Result<wake::Lease, String> {
        let now = epoch()?;
        let cadence = millis(self.clock.cadence())?;
        let first = now
            .checked_add(timing(self.clock.window())?)
            .ok_or_else(|| "wake lease first time is out of range".to_string())?;
        self.store
            .enable_wake(
                soul,
                i64::try_from(cadence)
                    .map_err(|_| "wake lease cadence is out of range".to_string())?,
                first,
                &crate::now(),
            )
            .await
            .and_then(project)
    }

    pub async fn disable_wake(&self, soul: &str) -> Result<wake::Lease, String> {
        if self.store.soul(soul).await?.is_none() {
            return Err("soul not found".to_string());
        }
        let cadence = millis(self.clock.cadence())?;
        self.store
            .disable_wake(soul, &crate::now())
            .await?
            .map(project)
            .transpose()
            .map(|lease| lease.unwrap_or_else(|| wake::Lease::revoked(soul, cadence)))
    }

    pub async fn renew_wake(&self, soul: &str, generation: u64) -> Result<wake::Lease, String> {
        let first = epoch()?
            .checked_add(timing(self.clock.window())?)
            .ok_or_else(|| "wake lease first time is out of range".to_string())?;
        self.store
            .renew_wake(soul, generation, first, &crate::now())
            .await
            .and_then(project)
    }

    pub async fn silence_wake(&self, soul: &str, generation: u64) -> Result<wake::Lease, String> {
        self.store
            .silence_wake(soul, generation, &crate::now())
            .await
            .and_then(project)
    }
}

pub(in crate::service) fn project(record: santi_estate::WakeLease) -> Result<wake::Lease, String> {
    let state = match record.state.as_str() {
        "active" => wake::State::Active,
        "expired" => wake::State::Expired,
        "revoked" => wake::State::Revoked,
        "silent" => wake::State::Silent,
        state => return Err(format!("unknown wake lease state {state}")),
    };
    Ok(wake::Lease {
        soul: record.soul,
        generation: record.generation,
        state,
        rounds_remaining: record.remaining,
        cadence_millis: record.cadence_millis,
        next_at: record.next_millis.map(stamp).transpose()?,
        last_wake_at: record.last_wake_millis.map(stamp).transpose()?,
        updated: Some(record.updated),
    })
}

fn stamp(millis: i64) -> Result<String, String> {
    let millis =
        u64::try_from(millis).map_err(|_| "wake lease time is out of range".to_string())?;
    crate::stamped(UNIX_EPOCH + Duration::from_millis(millis))
}

fn epoch() -> Result<i64, String> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    i64::try_from(millis).map_err(|_| "system clock is out of range".to_string())
}

fn millis(duration: Duration) -> Result<u64, String> {
    u64::try_from(duration.as_millis())
        .map_err(|_| "wake lease cadence is out of range".to_string())
}

fn timing(duration: Duration) -> Result<i64, String> {
    i64::try_from(duration.as_millis()).map_err(|_| "wake lease timing is out of range".to_string())
}
