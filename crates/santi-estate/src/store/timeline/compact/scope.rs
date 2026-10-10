use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Tx, form};

pub(super) struct Scope {
    pub(super) spans: Vec<Span>,
    pub(super) messages: Vec<Boundary>,
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
        let tag = text(&entry, "target")?;
        let row = tx
            .one(&form("Message").when("tag", Op::Eq, tag))
            .await?
            .ok_or_else(|| Error::Missing(tag.into()))?;
        messages.push(Boundary {
            key: row.key(),
            tag: tag.into(),
            sequence: number(&entry, "sequence")?,
            fixed: row.text("state") == Some("fixed")
                && matches!(row.text("actor_type"), Some("soul" | "system"))
                && matches!(row.text("kind"), Some("text" | "santi_system")),
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
    Ok(Scope { spans, messages })
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
