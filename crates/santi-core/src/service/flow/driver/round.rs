use santi_provider::{Receipt, Report};

use crate::thinking;

#[derive(Default)]
pub(super) struct Ledger {
    round: thinking::Round,
    anchor: Option<String>,
    heard: bool,
    filed: bool,
}

impl Ledger {
    pub(super) fn received(&mut self, receipt: Receipt) {
        self.heard = true;
        self.round.code = Some(receipt.status);
        self.round.millis = Some(receipt.elapsed);
        self.round.headers = receipt.headers.into_iter().collect();
    }

    pub(super) fn reported(&mut self, report: Report) {
        self.heard = true;
        let round = &mut self.round;
        round.status = report.status.or(round.status.take());
        round.incomplete = report.incomplete.or(round.incomplete.take());
        round.model = report.model.or(round.model.take());
        round.usage = report.usage.or(round.usage.take());
    }

    pub(super) fn retained(&mut self, state: thinking::Continuation) {
        self.heard = true;
        self.round.continuation = Some(Box::new(state));
    }

    pub(super) fn spoke(&mut self, delta: &str) {
        self.round.text += delta.len();
    }

    pub(super) fn called(&mut self, name: &str) {
        self.round.calls.push(name.to_string());
    }

    pub(super) fn anchored(&mut self, span: Option<&thinking::Span>) {
        if self.anchor.is_none() {
            self.anchor = span.map(|span| span.id.clone());
        }
    }

    pub(super) fn due(&mut self) -> Option<(String, thinking::Round)> {
        if self.filed || !self.heard {
            return None;
        }
        let anchor = self.anchor.clone()?;
        self.filed = true;
        Some((anchor, self.round.clone()))
    }
}
