mod apply;
pub(super) mod clock;
mod definition;
mod escalate;
mod maintenance;

use apply::asked;
pub(super) use definition::definition;
pub(super) use escalate::exhausted;

use crate::service::Service;

impl Service {
    pub(in crate::service) async fn crowded(&self, strand: &str) -> Result<Option<String>, String> {
        let policy = self.regimen();
        let Some(budget) = self.budget() else {
            return Ok(None);
        };
        let spans = self.spans(strand).await?;
        let live = spans
            .iter()
            .filter(|span| !crate::service::face::compact::slots::covered(span, &spans))
            .collect::<Vec<_>>();
        let settled = live.iter().map(|span| span.bytes).sum::<i64>();
        let held = live.len();
        let active = self.loaded(strand).await?.saturating_sub(settled);
        if settled > policy.settled as i64 {
            return Ok(Some(overfull(settled, policy.settled as i64, held)));
        }
        if held > policy.slots {
            return Ok(Some(crowded(held, policy.slots, settled)));
        }
        let ceiling = budget.bytes.saturating_sub(policy.settled as i64);
        if active <= ceiling {
            return Ok(None);
        }
        Ok(Some(pressed(active, ceiling, settled, held)))
    }
}

fn pressed(active: i64, ceiling: i64, settled: i64, held: usize) -> String {
    format!(
        "context refused: the live part of this strand is {active} bytes against a ceiling of {ceiling}, with {held} slots holding {settled} bytes already. Use the compact tool with summary alone to collapse settled history since the last slot, keeping the most recent message live. When all slots are occupied, supply absorb with two or more contiguous Compact IDs from projection headers to merge them. Ordinary tools return when the context fits."
    )
}

fn crowded(held: usize, ceiling: usize, settled: i64) -> String {
    format!(
        "context refused: {held} slots hold the compacted timeline against a ceiling of {ceiling}, carrying {settled} bytes. Use the compact tool with summary and absorb containing two or more contiguous Compact IDs from projection headers to merge slots. A precise range uses first and last Message IDs. Ordinary tools return when the context fits."
    )
}

fn overfull(settled: i64, ceiling: i64, held: usize) -> String {
    format!(
        "context refused: the compacted timeline carries {settled} bytes across {held} slots against a ceiling of {ceiling}. Use the compact tool with a shorter summary and absorb containing occupied Compact IDs from projection headers. One ID rewrites a slot; two or more contiguous IDs merge slots. Ordinary tools return when the context fits."
    )
}
