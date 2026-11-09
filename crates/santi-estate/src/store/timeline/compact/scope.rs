use super::thread::{self, Thread};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Tx, form};
use santi_model::compact;

pub(super) struct Scope {
    pub(super) spans: Vec<Span>,
    pub(super) messages: Vec<Boundary>,
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
}

pub(super) async fn load(tx: &mut Tx<'_, Sqlite>, strand: &str) -> Result<Scope, Error> {
    let key = crate::store::read::need(tx, "Strand", "tag", strand)
        .await?
        .to_string();
    let rows = tx
        .ask(
            &form("StrandEntry")
                .when("strand", Op::Eq, &key)
                .when("target_type", Op::Eq, "message")
                .order("sequence", keel::Rank::Asc),
        )
        .await?
        .rows()
        .to_vec();
    let mut messages = Vec::with_capacity(rows.len());
    for entry in rows {
        let row = crate::store::entry::aim(tx, &entry).await?;
        let tag = text(&row, "tag")?;
        messages.push(Boundary {
            key: row.key(),
            tag: tag.into(),
            sequence: number(&entry, "sequence")?,
            fixed: eligible(&row),
        });
    }
    let rows = tx
        .ask(&form("Compact").when("strand", Op::Eq, &key))
        .await?
        .rows()
        .to_vec();
    let mut spans = Vec::with_capacity(rows.len());
    for row in rows {
        let first = boundary(&messages, number(&row, "first")?)?;
        let last = boundary(&messages, number(&row, "last")?)?;
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
    Ok(Scope {
        spans,
        messages,
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

fn boundary(messages: &[Boundary], key: i64) -> Result<&Boundary, Error> {
    messages
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
            .messages
            .iter()
            .find(|message| message.tag == range.first);
        let last = self
            .messages
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
