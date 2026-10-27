use super::{Sqlite, int};
use keel::{Op, Rank, Row, Tx, form};
use std::collections::BTreeSet;

pub(super) async fn selected(
    tx: &mut Tx<'_, Sqlite>,
    parent: &Row,
    end: i64,
) -> Result<i64, keel::adapt::Error> {
    let entries = tx
        .ask(
            &form("StrandEntry")
                .when("strand", Op::Eq, &parent.key().to_string())
                .when("sequence", Op::Le, &end.to_string())
                .order("sequence", Rank::Asc),
        )
        .await?
        .rows()
        .to_vec();
    let mut at = end;
    let mut turns = BTreeSet::new();
    for entry in entries {
        let Some(table) = table(entry.text("target_type")) else {
            continue;
        };
        let column = crate::store::entry::column(entry.text("target_type").unwrap_or(""))?;
        let target = needed(tx, table, int(&entry, column)?).await?;
        let key = int(&target, "turn")?;
        if turns.insert(key) {
            at = at.min(boundary(tx, key, end).await?);
        }
    }
    Ok(at.max(0))
}

async fn boundary(tx: &mut Tx<'_, Sqlite>, key: i64, end: i64) -> Result<i64, keel::adapt::Error> {
    let completed = tx
        .one(&form("TurnCompletion").when("turn", Op::Eq, &key.to_string()))
        .await?;
    if completed
        .as_ref()
        .is_some_and(|row| row.int("to").is_some_and(|to| to <= end))
    {
        return Ok(end);
    }
    Ok(int(&needed(tx, "Turn", key).await?, "from")?.saturating_sub(1))
}

async fn needed(tx: &mut Tx<'_, Sqlite>, table: &str, key: i64) -> Result<Row, keel::adapt::Error> {
    tx.one(&form(table).when("id", Op::Eq, &key.to_string()))
        .await?
        .ok_or_else(|| keel::adapt::Error::Missing(format!("fork {table} {key}")))
}

fn table(kind: Option<&str>) -> Option<&'static str> {
    match kind {
        Some("thinking") => Some("ThinkingSpan"),
        Some("tool_call") => Some("ToolCall"),
        _ => None,
    }
}
