use super::{Sqlite, int};
use keel::{Op, Rank, Row, Tx, form};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Reader<'a, 'tx>(pub(super) &'a mut Tx<'tx, Sqlite>);

impl Reader<'_, '_> {
    pub(super) async fn selected(
        &mut self,
        parent: &Row,
        end: i64,
    ) -> Result<i64, keel::adapt::Error> {
        let entries = self
            .0
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
            let target = self.needed(table, int(&entry, column)?).await?;
            let key = int(&target, "turn")?;
            if turns.insert(key) {
                at = at.min(self.boundary(key, end).await?);
            }
        }
        Ok(at.max(0))
    }

    async fn boundary(&mut self, key: i64, end: i64) -> Result<i64, keel::adapt::Error> {
        let completed = self
            .0
            .one(&form("TurnCompletion").when("turn", Op::Eq, &key.to_string()))
            .await?;
        if completed
            .as_ref()
            .is_some_and(|row| row.int("to").is_some_and(|to| to <= end))
        {
            return Ok(end);
        }
        Ok(int(&self.needed("Turn", key).await?, "from")?.saturating_sub(1))
    }

    async fn needed(&mut self, table: &str, key: i64) -> Result<Row, keel::adapt::Error> {
        self.0
            .one(&form(table).when("id", Op::Eq, &key.to_string()))
            .await?
            .ok_or_else(|| keel::adapt::Error::Missing(format!("fork {table} {key}")))
    }

    pub(super) async fn position(
        &mut self,
        parent: &Row,
        at: i64,
    ) -> Result<i64, keel::adapt::Error> {
        let head = int(parent, "next")?.saturating_sub(1);
        if at < 0 {
            return refused(at, "must be >= 0", None, Some(0));
        }
        if at == 0 {
            return Ok(0);
        }
        let entries = self
            .0
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &parent.key().to_string())
                    .when("sequence", Op::Le, &head.to_string())
                    .order("sequence", Rank::Asc),
            )
            .await?
            .rows()
            .to_vec();
        let mut turns = BTreeMap::new();
        for entry in entries {
            let Some(table) = table(entry.text("target_type")) else {
                continue;
            };
            let column = crate::store::entry::column(entry.text("target_type").unwrap_or(""))?;
            let target = self.needed(table, int(&entry, column)?).await?;
            let key = int(&target, "turn")?;
            let sequence = int(&entry, "sequence")?;
            turns.entry(key).or_insert(sequence);
        }
        let mut spans = Vec::new();
        for (turn, start) in turns {
            let end = self
                .completed(turn)
                .await?
                .filter(|to| *to <= head)
                .map_or(head, |to| to - 1);
            if start <= end {
                spans.push((start, end));
            }
        }
        spans.sort_unstable();
        let mut merged: Vec<(i64, i64)> = Vec::new();
        for (start, end) in spans {
            match merged.last_mut() {
                Some(last) if start <= last.1 + 1 => last.1 = last.1.max(end),
                _ => merged.push((start, end)),
            }
        }
        if at > head {
            let last = merged.last().filter(|(_, end)| *end == head);
            let earlier = last.map_or(head, |(start, _)| start - 1);
            return refused(
                at,
                &format!("is past parent end {head}"),
                Some(earlier),
                None,
            );
        }
        match merged
            .into_iter()
            .find(|(start, end)| *start <= at && at <= *end)
        {
            Some((start, end)) => refused(
                at,
                "falls inside a turn, which must be inherited whole",
                start.checked_sub(1).filter(|at| *at >= 0),
                (end < head).then_some(end + 1),
            ),
            None => Ok(at),
        }
    }

    async fn completed(&mut self, turn: i64) -> Result<Option<i64>, keel::adapt::Error> {
        Ok(self
            .0
            .one(&form("TurnCompletion").when("turn", Op::Eq, &turn.to_string()))
            .await?
            .and_then(|row| row.int("to")))
    }
}

fn table(kind: Option<&str>) -> Option<&'static str> {
    match kind {
        Some("thinking") => Some("ThinkingSpan"),
        Some("tool_call") => Some("ToolCall"),
        _ => None,
    }
}

fn refused(
    at: i64,
    reason: &str,
    earlier: Option<i64>,
    later: Option<i64>,
) -> Result<i64, keel::adapt::Error> {
    let label = |position: Option<i64>| position.map_or_else(|| "none".into(), |at| at.to_string());
    Err(keel::adapt::Error::Adapt(format!(
        "fork {at} {reason}; the nearest earlier valid position is {} and the nearest later valid position is {}",
        label(earlier),
        label(later)
    )))
}
