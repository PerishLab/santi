use super::{
    Limits,
    scope::{Boundary, Scope, Span},
};
use santi_model::compact;

impl Scope {
    pub(super) fn select(
        &self,
        request: &compact::Exec,
        limits: Limits,
    ) -> Result<compact::Choice, String> {
        let summary = request.summary.trim();
        if summary.is_empty() {
            return Err("[compact.summary] compact summary must not be empty".into());
        }
        self.admit(request, limits)?;
        let input = Bounds {
            first: trimmed(request.first.as_deref()),
            last: trimmed(request.last.as_deref()),
            from: request.from,
            to: request.to,
        };
        let named = match input {
            Bounds {first: Some(first), last: Some(last), from: None, to: None} => Some((self.located("first", first)?, self.located("last", last)?)),
            Bounds {first: None, last: None, from: Some(from), to: Some(to)} => Some((self.seated("from", from)?, self.seated("to", to)?)),
            Bounds {first: None, last: None, from: None, to: None} => None,
            _ => return Err("[compact.form] compact takes first and last together as message ids, or from and to together as message sequences, never one end alone or a mixture; omit them all to compact the settled history since the last occupied slot".into()),
        };
        let slots = self.chosen(&request.absorb, named.is_none())?;
        let Some((first, last)) = named else {
            let range = match hull(&slots) {
                Some((from, to)) => self.resolve(from, to)?,
                None => self.settled()?,
            };
            return Ok(compact::Choice {
                range,
                requested: None,
            });
        };
        if first.sequence > last.sequence {
            return Err(self.reversed(first, last, &slots));
        }
        let range = self.reach(first, last, &slots)?;
        let requested = compact::Range {
            first: first.tag.clone(),
            last: last.tag.clone(),
        };
        Ok(compact::Choice {
            requested: (range != requested).then_some(requested),
            range,
        })
    }

    fn reach(
        &self,
        first: &Boundary,
        last: &Boundary,
        slots: &[&Span],
    ) -> Result<compact::Range, String> {
        let Some((from, to)) = self.inward(first.sequence, last.sequence) else {
            let named = slots
                .iter()
                .map(|span| span.compact.clone())
                .collect::<Vec<_>>();
            let absorb = match self.chosen(&named, true) {
                Ok(_) if !slots.is_empty() => format!(
                    "; to compact only the named slots, submit absorb: [{}] without first and last",
                    names(slots)
                ),
                _ => String::new(),
            };
            return Err(format!(
                "[compact.empty] no eligible boundary lies between {} and {}: a boundary must be fixed soul or system text, so pending and aborted messages never are; query messages to choose boundaries{absorb}",
                first.tag, last.tag
            ));
        };
        let (from, to) = match hull(slots) {
            Some((low, high)) => (from.min(low), to.max(high)),
            None => (from, to),
        };
        self.resolve(from, to)
    }

    fn reversed(&self, first: &Boundary, last: &Boundary, slots: &[&Span]) -> String {
        let swapped = match self.reach(last, first, slots) {
            Ok(range) => format!("; ready to submit: {}", argued(&range)),
            Err(_) => String::new(),
        };
        format!(
            "[compact.order] compact first {} (sequence {}) comes after last {} (sequence {}){swapped}",
            first.tag, first.sequence, last.tag, last.sequence
        )
    }

    fn resolve(&self, from: i64, to: i64) -> Result<compact::Range, String> {
        let partial = self.partial(from, to);
        if partial.is_empty() {
            return Ok(self.range(from, to));
        }
        let slots = partial
            .iter()
            .map(|span| format!("{} (covering {} to {})", span.compact, span.head, span.tail))
            .collect::<Vec<_>>()
            .join(", ");
        let mut forward = vec![format!(
            "{} (encloses and absorbs the slot)",
            argued(&self.enclosed(from, to))
        )];
        if let Some(range) = self.excluded(from, to) {
            forward.push(format!("{} (leaves the slot out)", argued(&range)));
        }
        Err(format!(
            "[compact.overlap] compact range {} to {} partially overlaps occupied slot {slots}; a range must enclose a slot or leave it out. Ready to submit: {}",
            self.tag(from),
            self.tag(to),
            forward.join("; or ")
        ))
    }

    fn enclosed(&self, mut from: i64, mut to: i64) -> compact::Range {
        while let Some(span) = self.partial(from, to).first() {
            from = from.min(span.from);
            to = to.max(span.to);
        }
        self.range(from, to)
    }

    fn excluded(&self, mut from: i64, mut to: i64) -> Option<compact::Range> {
        while let Some(span) = self.partial(from, to).first() {
            if span.from <= from {
                from = self.inward(span.to + 1, to)?.0;
            } else {
                to = self.inward(from, span.from - 1)?.1;
            }
        }
        Some(self.range(from, to))
    }

    fn partial(&self, from: i64, to: i64) -> Vec<&Span> {
        self.spans
            .iter()
            .filter(|span| span.overlaps(from, to) && !span.enclosed(from, to))
            .collect()
    }

    fn inward(&self, from: i64, to: i64) -> Option<(i64, i64)> {
        let mut eligible = self
            .messages
            .iter()
            .filter(|message| message.fixed && (from..=to).contains(&message.sequence));
        let first = eligible.next()?.sequence;
        let last = eligible
            .next_back()
            .map_or(first, |message| message.sequence);
        Some((first, last))
    }

    fn range(&self, from: i64, to: i64) -> compact::Range {
        compact::Range {
            first: self.tag(from),
            last: self.tag(to),
        }
    }

    fn tag(&self, sequence: i64) -> String {
        self.messages
            .iter()
            .find(|message| message.sequence == sequence)
            .map(|message| message.tag.clone())
            .unwrap_or_default()
    }

    fn located(&self, label: &str, tag: &str) -> Result<&Boundary, String> {
        self.messages
            .iter()
            .find(|message| message.tag == tag)
            .ok_or_else(|| format!("[compact.unknown] compact {label} {tag} is not a message in this strand; query slots or messages for current references"))
    }

    fn seated(&self, label: &str, sequence: i64) -> Result<&Boundary, String> {
        self.messages.iter().find(|message| message.sequence == sequence)
            .ok_or_else(|| format!("[compact.unknown] compact {label} {sequence} is not a message sequence in this strand; from/to take message sequences, while tool and turn records carry sequences of their own and are not messages; query messages to pick a boundary, or pass first/last with message ids instead"))
    }

    pub(super) fn settled(&self) -> Result<compact::Range, String> {
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
        settled
            .first()
            .zip(settled.last())
            .and_then(|(first, last)| self.inward(first.sequence, last.sequence))
            .map(|(from, to)| self.range(from, to))
            .ok_or_else(|| format!(
                "[compact.settled] compact has no settled range to collapse: no eligible boundary lies after sequence {floor} outside the live exchange, and the most recent message is always kept out of a default range; name first/last or absorb occupied slots to choose a range yourself"
            ))
    }

    fn chosen(&self, named: &[String], alone: bool) -> Result<Vec<&Span>, String> {
        let mut chosen = Vec::new();
        for name in named {
            let found = self.spans.iter().find(|span| span.compact == *name)
                .ok_or_else(|| format!("[compact.absorb] absorb names {name}, which is not an occupied slot on this strand; occupied slots are {}", names(&self.ordered())))?;
            chosen.push(found);
        }
        chosen.sort_by_key(|span| span.from);
        let missed = self
            .spans
            .iter()
            .filter(|span| between(span, &chosen))
            .map(|span| span.compact.clone())
            .collect::<Vec<_>>();
        if alone && !missed.is_empty() {
            return Err(format!(
                "[compact.absorb] absorb names a set that is not contiguous: {} lies between the slots you named and would be collapsed without being chosen; name it too, or choose adjacent slots",
                missed.join(", ")
            ));
        }
        Ok(chosen)
    }

    pub(super) fn ordered(&self) -> Vec<&Span> {
        let mut spans = self.spans.iter().collect::<Vec<_>>();
        spans.sort_by_key(|span| span.from);
        spans
    }
}

fn hull(slots: &[&Span]) -> Option<(i64, i64)> {
    let from = slots.iter().map(|span| span.from).min()?;
    let to = slots.iter().map(|span| span.to).max()?;
    Some((from, to))
}

pub(super) fn names(spans: &[&Span]) -> String {
    spans
        .iter()
        .map(|span| span.compact.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn argued(range: &compact::Range) -> String {
    format!("first: {}, last: {}", range.first, range.last)
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
