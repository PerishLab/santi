use crate::service::Service;

impl Service {
    pub(super) async fn crowded(&self, strand: &str) -> Result<Option<String>, String> {
        let policy = self.regimen();
        let Some(slots) = self.occupancy(strand).await? else {
            return Ok(None);
        };
        let Some(budget) = self.budget() else {
            return Ok(None);
        };
        let settled = slots.held.iter().map(|slot| slot.bytes).sum::<i64>();
        let held = slots.held.len();
        if held > policy.slots {
            return Ok(Some(crowded(held, policy.slots, settled)));
        }
        let ceiling = budget
            .bytes
            .saturating_sub((policy.slots as i64).saturating_mul(policy.slot as i64));
        if slots.active <= ceiling {
            return Ok(None);
        }
        Ok(Some(pressed(slots.active, ceiling, settled, held)))
    }
}

fn pressed(active: i64, ceiling: i64, settled: i64, held: usize) -> String {
    format!(
        "context refused: the live part of this strand is {active} bytes against a ceiling of {ceiling}, with {held} slots holding {settled} bytes already. Nothing ordinary proceeds until it fits. Run `santi compact exec --summary <text>` to collapse everything settled since the last slot; the most recent message stays live. With every slot occupied, name a range across two of them so the compaction absorbs them into one."
    )
}

fn crowded(held: usize, ceiling: usize, settled: i64) -> String {
    format!(
        "context refused: {held} slots hold the compacted timeline against a ceiling of {ceiling}, carrying {settled} bytes. Nothing ordinary proceeds until it fits. Every compaction from here must absorb: name first/last or from/to spanning two or more occupied slots so they collapse into one, and decide what survives and how the surviving summary reads. Read the slot occupancy beside the budget to choose which to merge."
    )
}
