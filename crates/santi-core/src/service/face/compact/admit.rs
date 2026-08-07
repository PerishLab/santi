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
        if summary.len() > policy.slot {
            return Err(oversized(summary.len(), policy.slot));
        }
        let spans = self.spans(strand).await?;
        let held = spans.iter().filter(|span| !covered(span, &spans)).count();
        if held < policy.slots || absorbing(request) {
            return Ok(());
        }
        Err(crowded(held, policy.slots))
    }
}

fn absorbing(request: &compact::Exec) -> bool {
    request.first.is_some() || request.from.is_some()
}

fn oversized(weight: usize, ceiling: usize) -> String {
    format!(
        "compact summary is {weight} bytes and a slot holds at most {ceiling}; a summary that does not fit is not a compaction but a copy, so shorten it or split the range"
    )
}

fn crowded(held: usize, ceiling: usize) -> String {
    format!(
        "compact refused: {held} of {ceiling} slots are occupied and a default range would open another; name first/last or from/to across two or more occupied slots so this compaction absorbs them into one, deciding what survives and how the surviving summary reads"
    )
}
