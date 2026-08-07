use std::collections::HashMap;

use crate::budget;

use crate::service::Service;

pub(in crate::service) struct Span {
    pub(in crate::service) from: i64,
    pub(in crate::service) to: i64,
    compact: String,
    pub(in crate::service) bytes: i64,
}

pub(in crate::service) fn covered(span: &Span, spans: &[Span]) -> bool {
    spans.iter().any(|other| {
        other.compact != span.compact && other.from <= span.from && other.to >= span.to
    })
}

impl Service {
    pub(in crate::service) async fn spans(&self, strand: &str) -> Result<Vec<Span>, String> {
        let entries = self.store.entries(strand).await?;
        let seats = entries
            .iter()
            .filter(|entry| entry.kind == crate::strand::Target::Message)
            .map(|entry| (entry.target.as_str(), entry.seq))
            .collect::<HashMap<_, _>>();
        let mut spans = Vec::new();
        for compact in self.store.compacts(strand).await? {
            let from = seats.get(compact.first.as_str()).copied();
            let to = seats.get(compact.last.as_str()).copied();
            let (Some(from), Some(to)) = (from, to) else {
                continue;
            };
            spans.push(Span {
                from,
                to,
                compact: compact.id,
                bytes: compact.summary.len() as i64,
            });
        }
        Ok(spans)
    }

    pub(in crate::service) async fn occupancy(
        &self,
        strand: &str,
    ) -> Result<Option<budget::Slots>, String> {
        let policy = self.regimen();
        let spans = self.spans(strand).await?;
        let held = spans
            .iter()
            .filter(|span| !covered(span, &spans))
            .map(|span| budget::Slot {
                compact: span.compact.clone(),
                bytes: span.bytes,
            })
            .collect::<Vec<_>>();
        let estimate = self.estimate(strand).await?;
        let settled = held.iter().map(|slot| slot.bytes).sum::<i64>();
        Ok(Some(budget::Slots {
            ceiling: policy.slot as i64,
            count: policy.slots as i64,
            free: (policy.slots as i64).saturating_sub(held.len() as i64),
            active: estimate.input.saturating_sub(settled),
            held,
        }))
    }
}
