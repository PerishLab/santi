use super::thread::{self, Thread};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Tx, form};
use santi_model::compact;

pub(super) struct Scope {
    pub(super) spans: Vec<Span>,
    pub(super) boundaries: Vec<Boundary>,
    pub(super) thread: Thread,
}

#[derive(Clone)]
pub(super) struct Span {
    pub(super) from: i64,
    pub(super) to: i64,
    pub(super) compact: String,
    pub(super) bytes: usize,
    pub(super) head: String,
    pub(super) tail: String,
}

pub(super) struct Boundary {
    pub(super) key: i64,
    pub(super) tag: String,
    pub(super) sequence: i64,
    pub(super) fixed: bool,
    pub(super) message: bool,
}

pub(super) async fn load(tx: &mut Tx<'_, Sqlite>, strand: &str) -> Result<Scope, Error> {
    let key = crate::store::read::need(tx, "Strand", "tag", strand)
        .await?
        .to_string();
    let rows = tx
        .ask(
            &form("StrandEntry")
                .when("strand", Op::Eq, &key)
                .order("sequence", keel::Rank::Asc),
        )
        .await?
        .rows()
        .to_vec();
    let mut boundaries = Vec::with_capacity(rows.len());
    for entry in rows {
        let row = crate::store::entry::aim(tx, &entry).await?;
        let tag = text(&row, "tag")?;
        boundaries.push(Boundary {
            key: entry.key(),
            tag: tag.into(),
            sequence: number(&entry, "sequence")?,
            fixed: if entry.text("target_type") == Some("message") {
                eligible(&row)
            } else {
                matches!(entry.text("target_type"), Some("thinking" | "tool_result"))
            },
            message: entry.text("target_type") == Some("message"),
        });
    }
    let rows = tx
        .ask(&form("Compact").when("strand", Op::Eq, &key))
        .await?
        .rows()
        .to_vec();
    let mut spans = Vec::with_capacity(rows.len());
    for row in rows {
        let (head, tail) = super::boundary::Reader(tx).ends(&row).await?;
        let first = boundary(&boundaries, head.key())?;
        let last = boundary(&boundaries, tail.key())?;
        spans.push(Span {
            from: first.sequence,
            to: last.sequence,
            compact: text(&row, "tag")?.into(),
            bytes: text(&row, "summary")?.len(),
            head: first.tag.clone(),
            tail: last.tag.clone(),
        });
    }
    let spans = spans
        .iter()
        .filter(|span| !covered(span, &spans))
        .cloned()
        .collect();
    let thread = thread::load(tx, strand).await?;
    let endpoints = thread.endpoints();
    for message in &mut boundaries {
        if !message.message {
            message.fixed &= endpoints.contains(&message.sequence);
        }
    }
    Ok(Scope {
        spans,
        boundaries,
        thread,
    })
}

pub(super) fn eligible(message: &keel::Row) -> bool {
    message.text("state") == Some("fixed")
        && matches!(message.text("actor_type"), Some("soul" | "system"))
        && matches!(message.text("kind"), Some("text" | "santi_system"))
}

fn covered(span: &Span, spans: &[Span]) -> bool {
    spans.iter().any(|other| {
        other.compact != span.compact && other.from <= span.from && other.to >= span.to
    })
}

fn boundary(boundaries: &[Boundary], key: i64) -> Result<&Boundary, Error> {
    boundaries
        .iter()
        .find(|message| message.key == key)
        .ok_or_else(|| Error::Missing("compact boundary message".into()))
}

fn text<'a>(row: &'a keel::Row, field: &str) -> Result<&'a str, Error> {
    row.text(field)
        .ok_or_else(|| Error::Adapt(format!("compact {field} missing")))
}

fn number(row: &keel::Row, field: &str) -> Result<i64, Error> {
    row.int(field)
        .ok_or_else(|| Error::Adapt(format!("compact {field} missing")))
}

impl Scope {
    pub(super) fn settled(&self) -> Result<compact::Range, String> {
        let floor = self.spans.iter().map(|span| span.to).max().unwrap_or(0);
        let latest = self
            .boundaries
            .iter()
            .rfind(|entry| entry.message)
            .map(|entry| entry.sequence);
        let candidates = self
            .boundaries
            .iter()
            .filter(|entry| entry.fixed && entry.sequence > floor && Some(entry.sequence) != latest)
            .map(|entry| entry.sequence)
            .collect::<Vec<_>>();
        let range = match latest {
            Some(latest) => {
                let before = candidates
                    .iter()
                    .copied()
                    .filter(|at| *at < latest)
                    .collect::<Vec<_>>();
                let after = candidates
                    .iter()
                    .copied()
                    .filter(|at| *at > latest)
                    .collect::<Vec<_>>();
                self.window(&before).or_else(|| self.window(&after))
            }
            None => self.window(&candidates),
        };
        range
            .map(|(from, to)| self.range(from, to))
            .ok_or_else(|| format!(
                "[compact.settled] compact has no settled range to collapse: no eligible boundary lies after sequence {floor} outside the live exchange and at whole recorded tool-exchange boundaries, and the most recent message is always kept out of a default range; name first/last or absorb occupied slots to choose a range yourself"
            ))
    }

    pub(super) fn window(&self, candidates: &[i64]) -> Option<(i64, i64)> {
        for start in candidates {
            if let Some(range) = self.narrowed(*start, candidates) {
                return Some(range);
            }
        }
        None
    }

    fn narrowed(&self, start: i64, candidates: &[i64]) -> Option<(i64, i64)> {
        let holes = self.thread.holes(start)?;
        let mut end = *candidates.last()?;
        while let Some(low) = covering(&holes, end) {
            let previous = candidates.iter().rfind(|seat| **seat < low)?;
            if *previous < start {
                return None;
            }
            end = *previous;
        }
        Some((start, end))
    }

    pub(super) fn checked(&self, range: &compact::Range) -> Result<(), String> {
        let first = self
            .boundaries
            .iter()
            .find(|message| message.tag == range.first);
        let last = self
            .boundaries
            .iter()
            .find(|message| message.tag == range.last);
        match (first, last) {
            (Some(first), Some(last)) => match self.thread.split(first.sequence, last.sequence) {
                Some(reason) => Err(reason),
                None => Ok(()),
            },
            _ => Ok(()),
        }
    }
}

fn covering(holes: &[(i64, i64)], end: i64) -> Option<i64> {
    holes
        .iter()
        .find(|(low, high)| *low <= end && end <= *high)
        .map(|(low, _)| *low)
}
