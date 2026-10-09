use crate::message;

use super::Observed;

impl Observed {
    fn share(&self) -> Option<usize> {
        self.ceiling
            .filter(|ceiling| *ceiling > 0)
            .map(|ceiling| self.total().saturating_mul(100) / ceiling)
    }

    fn headroom(&self) -> Option<usize> {
        self.ceiling
            .map(|ceiling| ceiling.saturating_sub(self.total()))
    }
}

fn urged(band: &str) -> &'static str {
    match band {
        "slots" => {
            "Every compact slot is occupied. Merge two or more contiguous occupied Compact IDs into one summary to free a slot. Compact projection headers identify the slots and their covered Message IDs."
        }
        "hard" => {
            "Little headroom remains. Compact settled context through `santi compact exec` while shell is available, or the compact tool during context maintenance."
        }
        "firm" => {
            "Headroom is shrinking. Compacting settled context now costs less than compacting under pressure later."
        }
        _ => {
            "This strand is getting large. Compact settled context when useful; the summary preserves the facts and references you need to continue."
        }
    }
}

fn measured(event: &Observed) -> Vec<String> {
    let mut out = vec![format!("total_input_bytes: {}", event.total())];
    let Some(ceiling) = event.ceiling else {
        out.push(format!("reference_threshold_bytes: {}", event.threshold));
        return out;
    };
    out.push(format!("budget_bytes: {ceiling}"));
    if let Some(share) = event.share() {
        out.push(format!("share_percent: {share}"));
    }
    if let Some(headroom) = event.headroom() {
        out.push(format!("headroom_bytes: {headroom}"));
    }
    out.push(format!("band_floor_bytes: {}", event.threshold));
    out
}

pub(super) fn reminded(event: &Observed) -> message::Content {
    let mut lines = vec![
        "<system_message>".to_string(),
        "kind: compact_reminder".to_string(),
        "scope: strand_local".to_string(),
        "wake: false".to_string(),
        "obligation: false".to_string(),
        format!("band: {}", event.band),
        format!("trigger_turn_id: {}", event.address.turn),
        format!("round: {}", event.round),
        format!("provider: {}", event.provider),
        format!("model: {}", event.model),
        format!("items: {}", event.items),
        format!("input: {}", event.input),
        format!("instructions: {}", event.instructions),
    ];
    lines.push(format!("slots_held: {}", event.held));
    lines.push(format!("slots_ceiling: {}", event.slots));
    lines.extend(measured(event));
    lines.push(format!("summary: {}", urged(&event.band)));
    lines.push("</system_message>".to_string());
    message::Content::text(lines.join(
        "
",
    ))
}
