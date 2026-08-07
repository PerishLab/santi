use std::time::Duration;

use tokio::time::Instant;

use super::Presentation;

pub(super) const SILENCE: Duration = Duration::from_secs(60);
const LIMIT: Duration = Duration::from_secs(10 * 60);

pub(super) fn ceiling(presentation: Presentation, started: Instant) -> Option<Instant> {
    match presentation {
        Presentation::Watch(_) => Some(started + LIMIT),
        Presentation::Tui => None,
    }
}

pub(super) fn boundary(started: Instant, limit: Option<Instant>) -> &'static str {
    if limit.is_some() && started.elapsed() >= LIMIT {
        "watch reached its ten-minute proof limit"
    } else {
        "event stream produced no event for sixty seconds"
    }
}
