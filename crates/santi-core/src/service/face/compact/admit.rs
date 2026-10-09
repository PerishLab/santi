use crate::compact;
use crate::service::Service;

use super::slots::covered;

impl Service {
    pub(in crate::service::face) async fn admitted(
        &self,
        strand: &str,
        summary: &str,
        request: &compact::Exec,
    ) -> Result<(), String> {
        let policy = self.regimen();
        let spans = self.spans(strand).await?;
        let live = spans
            .iter()
            .filter(|span| !covered(span, &spans))
            .collect::<Vec<_>>();
        let carried = live.iter().map(|span| span.bytes as usize).sum::<usize>();
        if carried.saturating_add(summary.len()) > policy.settled && !absorbing(request) {
            return Err(oversized(carried, summary.len(), policy.settled));
        }
        if live.len() < policy.slots || absorbing(request) {
            return Ok(());
        }
        Err(crowded(live.len(), policy.slots))
    }
}

fn absorbing(request: &compact::Exec) -> bool {
    !request.absorb.is_empty() || request.first.is_some() || request.from.is_some()
}

fn oversized(carried: usize, weight: usize, ceiling: usize) -> String {
    format!(
        "compact refused: the settled part of this strand already carries {carried} bytes and this summary adds {weight}, past the {ceiling} the compacted timeline may hold; write a shorter summary, or absorb existing slots so this one replaces rather than adds"
    )
}

fn crowded(held: usize, ceiling: usize) -> String {
    format!(
        "compact refused: {held} of {ceiling} slots are occupied and a default range would open another; supply absorb with two or more contiguous occupied Compact IDs and a summary to merge them into one"
    )
}
