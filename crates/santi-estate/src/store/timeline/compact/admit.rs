use super::{Limits, scope::Scope};
use santi_model::compact;

impl Scope {
    pub(super) fn admit(&self, request: &compact::Exec, limits: Limits) -> Result<(), String> {
        let carried = self.spans.iter().map(|span| span.bytes).sum::<usize>();
        let weight = request.summary.trim().len();
        let absorbing =
            !request.absorb.is_empty() || request.first.is_some() || request.from.is_some();
        if carried.saturating_add(weight) > limits.settled && !absorbing {
            return Err(format!(
                "[compact.bytes] compact refused: the settled part of this strand already carries {carried} bytes and this summary adds {weight}, past the {} the compacted timeline may hold; write a shorter summary, or absorb existing slots so this one replaces rather than adds",
                limits.settled
            ));
        }
        if self.spans.len() < limits.slots || absorbing {
            return Ok(());
        }
        let ordered = self.ordered();
        Err(format!(
            "[compact.slots] compact refused: {} of {} slots are occupied and a default range would open another; supply absorb with two or more adjacent occupied slots and a summary to merge them into one. Occupied slots in order: {}; ready to submit, for example: absorb: [{}]",
            self.spans.len(),
            limits.slots,
            super::selection::names(&ordered),
            super::selection::names(&ordered[..ordered.len().min(2)])
        ))
    }
}
