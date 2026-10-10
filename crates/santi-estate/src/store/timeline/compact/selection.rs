use super::{
    Limits,
    scope::{Scope, Span},
};
use santi_model::compact;

impl Scope {
    pub(super) fn select(
        &self,
        request: &compact::Exec,
        limits: Limits,
    ) -> Result<(String, String), String> {
        let summary = request.summary.trim();
        if summary.is_empty() {
            return Err("compact summary must not be empty".into());
        }
        self.admit(request, limits)?;
        let input = Bounds {
            first: trimmed(request.first.as_deref()),
            last: trimmed(request.last.as_deref()),
            from: request.from,
            to: request.to,
        };
        let bounds = match input {
            Bounds {first: Some(first), last: Some(last), from: None, to: None} => (first.into(), last.into()),
            Bounds {first: None, last: None, from: Some(from), to: Some(to)} => (self.seated("from", from)?, self.seated("to", to)?),
            Bounds {first: None, last: None, from: None, to: None} if !request.absorb.is_empty() => self.merged(&request.absorb)?,
            Bounds {first: None, last: None, from: None, to: None} => self.settled()?,
            _ => return Err("compact requires either first/last as message ids or from/to as message sequences, and never a mixture of the two".into()),
        };
        self.validate(&bounds)?;
        Ok(bounds)
    }

    fn validate(&self, bounds: &(String, String)) -> Result<(), String> {
        let first = self.message(&bounds.0, "from")?;
        let last = self.message(&bounds.1, "to")?;
        if first > last {
            return Err("compact from must not be after to".into());
        }
        let partial = self
            .spans
            .iter()
            .any(|span| span.overlaps(first, last) && !span.enclosed(first, last));
        if partial {
            return Err("compact range partially overlaps an existing compact".into());
        }
        Ok(())
    }

    fn message(&self, tag: &str, label: &str) -> Result<i64, String> {
        let message = self
            .messages
            .iter()
            .find(|message| message.tag == tag)
            .ok_or_else(|| format!("compact {label} message not in this strand"))?;
        if !message.fixed {
            return Err(format!(
                "compact {label} boundary must be a fixed projected message"
            ));
        }
        Ok(message.sequence)
    }

    fn seated(&self, label: &str, sequence: i64) -> Result<String, String> {
        self.messages.iter().find(|message| message.sequence == sequence).map(|message| message.tag.clone())
            .ok_or_else(|| format!("compact {label} {sequence} is not a message sequence in this strand; from/to take message sequences, while tool and turn records carry sequences of their own and are not messages; read the strand's messages to pick a boundary, or pass first/last with message ids instead"))
    }

    fn settled(&self) -> Result<(String, String), String> {
        let floor = self.spans.iter().map(|span| span.to).max().unwrap_or(0);
        let loose = self
            .messages
            .iter()
            .filter(|message| message.sequence > floor)
            .collect::<Vec<_>>();
        let settled = loose
            .split_last()
            .map(|(_, settled)| settled)
            .unwrap_or_default();
        match (settled.first(), settled.last()) {
            (Some(first), Some(last)) => Ok((first.tag.clone(), last.tag.clone())),
            _ => Err(format!(
                "compact has no settled range to collapse: every message after sequence {floor} is still the live exchange, and the most recent message is always kept out of a default range; name first/last or from/to to choose a range yourself"
            )),
        }
    }

    fn merged(&self, named: &[String]) -> Result<(String, String), String> {
        let mut chosen = Vec::new();
        for name in named {
            let found = self.spans.iter().find(|span| span.compact == *name)
                .ok_or_else(|| format!("absorb names {name}, which is not an occupied slot on this strand; read the slot occupancy beside the budget for the ids that are"))?;
            chosen.push(found);
        }
        chosen.sort_by_key(|span| span.from);
        let first = chosen
            .first()
            .ok_or_else(|| "absorb names no slot".to_string())?;
        let last = chosen
            .last()
            .ok_or_else(|| "absorb names no slot".to_string())?;
        let missed = self
            .spans
            .iter()
            .filter(|span| between(span, &chosen))
            .map(|span| span.compact.clone())
            .collect::<Vec<_>>();
        if !missed.is_empty() {
            return Err(format!(
                "absorb names a set that is not contiguous: {} lies between the slots you named and would be collapsed without being chosen; name it too, or choose adjacent slots",
                missed.join(", ")
            ));
        }
        Ok((first.head.clone(), last.tail.clone()))
    }
}

fn between(span: &Span, chosen: &[&Span]) -> bool {
    let (Some(low), Some(high)) = (chosen.first(), chosen.last()) else {
        return false;
    };
    span.from > low.from
        && span.to < high.to
        && !chosen.iter().any(|held| held.compact == span.compact)
}

fn trimmed(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

struct Bounds<'a> {
    first: Option<&'a str>,
    last: Option<&'a str>,
    from: Option<i64>,
    to: Option<i64>,
}

impl Span {
    fn overlaps(&self, first: i64, last: i64) -> bool {
        self.to >= first && self.from <= last
    }
    fn enclosed(&self, first: i64, last: i64) -> bool {
        first <= self.from && self.to <= last
    }
}
