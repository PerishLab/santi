use keel::adapt::db::Sqlite;
use keel::{Op, Tx, form};

pub(in crate::store) async fn strand(
    tx: &mut Tx<'_, Sqlite>,
    tag: &str,
    fields: &[(&str, &str)],
) -> Result<i64, keel::adapt::Error> {
    if tx
        .one(&form("Retirement").when("tag", Op::Eq, tag))
        .await?
        .is_some()
    {
        return Err(keel::adapt::Error::Adapt(format!(
            "strand {tag} has already retired"
        )));
    }
    tx.put("Strand", fields).await
}

pub(in crate::store) async fn compact(
    tx: &mut Tx<'_, Sqlite>,
    tag: &str,
    fields: &[(&str, &str)],
) -> Result<i64, keel::adapt::Error> {
    if tx
        .one(&form("RetiredCompact").when("tag", Op::Eq, tag))
        .await?
        .is_some()
    {
        return Err(keel::adapt::Error::Adapt(format!(
            "compact {tag} has already retired"
        )));
    }
    tx.put("Compact", fields).await
}

pub(in crate::store) struct Entry<'a> {
    pub strand: &'a keel::Row,
    pub kind: &'a str,
    pub target: i64,
    pub created: &'a str,
}

impl<'a> Entry<'a> {
    pub fn new(strand: &'a keel::Row, kind: &'a str, target: i64, created: &'a str) -> Self {
        Self {
            strand,
            kind,
            target,
            created,
        }
    }
}

pub(in crate::store) async fn append(
    tx: &mut Tx<'_, Sqlite>,
    entry: Entry<'_>,
) -> Result<i64, keel::adapt::Error> {
    let sequence = entry
        .strand
        .int("next")
        .ok_or_else(|| keel::adapt::Error::Adapt("strand next missing".into()))?;
    tx.put(
        "StrandEntry",
        &[
            ("target_type", entry.kind),
            (super::entry::column(entry.kind)?, &entry.target.to_string()),
            ("sequence", &sequence.to_string()),
            ("created", entry.created),
            ("strand", &entry.strand.key().to_string()),
        ],
    )
    .await?;
    tx.set(
        "Strand",
        entry.strand.key(),
        &[
            ("next", &(sequence + 1).to_string()),
            ("updated", entry.created),
        ],
    )
    .await?;
    Ok(sequence)
}
