use keel::{Op, Row, form};

use super::WakeLease;
use crate::store::read;

pub(super) async fn lease(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    soul: &str,
) -> Result<Row, keel::adapt::Error> {
    let soul = read::need(tx, "Soul", "tag", soul).await?;
    tx.one(&form("WakeLease").when("soul", Op::Eq, &soul.to_string()))
        .await?
        .ok_or_else(|| adapt("wake lease not found"))
}

pub(super) fn expected(row: &Row, generation: u64) -> Result<(), keel::adapt::Error> {
    let current = unsigned(row, "generation")?;
    if current == generation {
        Ok(())
    } else {
        Err(adapt(format!(
            "wake lease generation conflicts: expected {generation}, current {current}"
        )))
    }
}

pub(super) fn advance(row: &Row) -> Result<i64, keel::adapt::Error> {
    row.int("generation")
        .ok_or_else(|| adapt("wake lease generation missing"))?
        .checked_add(1)
        .ok_or_else(|| adapt("wake lease generation is out of range"))
}

pub(super) fn unsigned(row: &Row, field: &str) -> Result<u64, keel::adapt::Error> {
    let value = row
        .int(field)
        .ok_or_else(|| adapt(format!("wake lease {field} missing")))?;
    u64::try_from(value).map_err(|_| adapt(format!("wake lease {field} is out of range")))
}

pub(super) async fn decode(
    core: &keel::Core<keel::adapt::db::Sqlite>,
    row: &Row,
) -> Result<WakeLease, String> {
    Ok(WakeLease {
        soul: read::related(core, "Soul", read::int(row, "soul")?).await?,
        generation: unsigned(row, "generation").map_err(|error| error.to_string())?,
        state: read::text(row, "state")?.to_string(),
        remaining: u8::try_from(read::int(row, "remaining")?)
            .map_err(|_| "wake lease remaining rounds are out of range".to_string())?,
        cadence_millis: unsigned(row, "cadence_millis").map_err(|error| error.to_string())?,
        next_millis: row.int("next_millis"),
        last_wake_millis: row.int("last_wake_millis"),
        created: read::text(row, "created")?.to_string(),
        updated: read::text(row, "updated")?.to_string(),
    })
}

pub(super) fn adapt(message: impl Into<String>) -> keel::adapt::Error {
    keel::adapt::Error::Adapt(message.into())
}
