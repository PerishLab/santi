use crate::message;
use crate::service::tools::Gauge;

use super::{Band, Observed, REFERENCE};

fn urged(band: Band) -> &'static str {
    match band {
        Band::Slots => {
            "Every compact slot is occupied. Merge two or more contiguous occupied Compact IDs into one summary to free a slot. Compact projection headers identify the slots and their covered Message IDs."
        }
        Band::Hard => {
            "Little headroom remains before ordinary tools are withheld for context maintenance. Compact settled context through `santi compact exec` while shell is available."
        }
        Band::Firm => {
            "Headroom is shrinking. Compacting settled context now costs less than compacting under pressure later."
        }
        _ => {
            "This strand is getting large. Compact settled context when useful; the summary preserves the facts and references you need to continue."
        }
    }
}

fn measured(gauge: &Gauge) -> Vec<String> {
    let mut out = vec![
        format!("estimator: {}", gauge.estimator),
        format!("active_bytes: {}", gauge.active),
    ];
    match gauge.ceiling {
        Some(ceiling) => {
            out.push(format!("active_ceiling_bytes: {ceiling}"));
            if ceiling > 0 {
                out.push(format!(
                    "share_percent: {}",
                    gauge.active.saturating_mul(100) / ceiling
                ));
            }
            out.push(format!(
                "headroom_bytes: {}",
                ceiling.saturating_sub(gauge.active)
            ));
        }
        None => out.push(format!("reference_threshold_bytes: {REFERENCE}")),
    }
    out.push(format!("settled_bytes: {}", gauge.settled));
    out.push(format!("slots_held: {}", gauge.held));
    out.push(format!("slots_ceiling: {}", gauge.slots));
    out
}

pub(super) fn reminded(event: &Observed, gauge: &Gauge, band: Band) -> message::Content {
    let mut lines = vec![
        "<system_message>".to_string(),
        "kind: compact_reminder".to_string(),
        "scope: strand_local".to_string(),
        "wake: false".to_string(),
        "obligation: false".to_string(),
        format!("band: {}", band.name()),
        format!("trigger_turn_id: {}", event.address.turn),
        format!("round: {}", event.round),
        format!("provider: {}", event.provider),
        format!("model: {}", event.model),
    ];
    lines.extend(measured(gauge));
    lines.push(format!("summary: {}", urged(band)));
    lines.push("</system_message>".to_string());
    message::Content::text(lines.join("\n"))
}
