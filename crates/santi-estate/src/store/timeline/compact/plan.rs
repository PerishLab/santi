use keel::adapt::db::Sqlite;
use keel::{Op, Tx, form};
use santi_model::compact;

pub(super) struct Plan {
    pub(super) strand: i64,
    pub(super) first: i64,
    pub(super) last: i64,
    pub(super) first_tag: String,
    pub(super) last_tag: String,
    pub(super) from: i64,
    pub(super) to: i64,
    pub(super) absorbed: Vec<(i64, String)>,
    pub(super) collapsed: i64,
}

impl Plan {
    pub(super) fn report(&self, tag: &str, dry: bool) -> compact::Report {
        compact::Report {
            compact: tag.to_string(),
            first: self.first_tag.clone(),
            last: self.last_tag.clone(),
            from: self.from,
            to: self.to,
            absorbed: self.absorbed.iter().map(|(_, tag)| tag.clone()).collect(),
            collapsed: self.collapsed,
            dry,
            active_incident_resolved: false,
            before: None,
            after: None,
            ratio: None,
            requested: None,
        }
    }

    pub(super) fn matches(&self, expected: &compact::Report) -> bool {
        let absorbed = self
            .absorbed
            .iter()
            .map(|(_, tag)| tag.as_str())
            .collect::<Vec<_>>();
        let held = expected
            .absorbed
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        Extent {
            first: self.first_tag.as_str(),
            last: self.last_tag.as_str(),
            from: self.from,
            to: self.to,
            absorbed,
            collapsed: self.collapsed,
        } == Extent {
            first: expected.first.as_str(),
            last: expected.last.as_str(),
            from: expected.from,
            to: expected.to,
            absorbed: held,
            collapsed: expected.collapsed,
        }
    }
}

#[derive(PartialEq)]
struct Extent<'a> {
    first: &'a str,
    last: &'a str,
    from: i64,
    to: i64,
    absorbed: Vec<&'a str>,
    collapsed: i64,
}

pub(super) async fn build(
    tx: &mut Tx<'_, Sqlite>,
    strand: &str,
    first: &str,
    last: &str,
) -> Result<Plan, keel::adapt::Error> {
    let scope = super::scope::load(tx, strand).await?;
    let head = boundary(&scope, first, "from")?;
    let tail = boundary(&scope, last, "to")?;
    let from = head.sequence;
    let to = tail.sequence;
    let strand = crate::store::read::need(tx, "Strand", "tag", strand).await?;
    if from > to {
        return Err(keel::adapt::Error::Adapt(
            "[compact.order] compact from must not be after to".into(),
        ));
    }
    if let Some(reason) = scope.thread.split(from, to) {
        return Err(keel::adapt::Error::Adapt(reason));
    }
    let mut absorbed = Vec::new();
    let rows = tx
        .ask(&form("Compact").when("strand", Op::Eq, &strand.to_string()))
        .await?
        .rows()
        .to_vec();
    for compact in rows {
        let (head, tail) = super::boundary::Reader(tx).ends(&compact).await?;
        let start = super::boundary::number(&head, "sequence")?;
        let end = super::boundary::number(&tail, "sequence")?;
        if end < from || start > to {
            continue;
        }
        if from <= start && end <= to {
            absorbed.push((
                compact.key(),
                crate::store::read::text(&compact, "tag")
                    .map_err(keel::adapt::Error::Adapt)?
                    .to_string(),
            ));
            continue;
        }
        return Err(keel::adapt::Error::Adapt(
            "[compact.overlap] compact range partially overlaps an existing compact".into(),
        ));
    }
    let collapsed = tx
        .ask(
            &form("StrandEntry")
                .when("strand", Op::Eq, &strand.to_string())
                .when("sequence", Op::Ge, &from.to_string())
                .when("sequence", Op::Le, &to.to_string()),
        )
        .await?
        .rows()
        .len() as i64;
    Ok(Plan {
        strand,
        first: head.key,
        last: tail.key,
        first_tag: head.tag.clone(),
        last_tag: tail.tag.clone(),
        from,
        to,
        absorbed,
        collapsed,
    })
}

fn boundary<'a>(
    scope: &'a super::scope::Scope,
    tag: &str,
    label: &str,
) -> Result<&'a super::scope::Boundary, keel::adapt::Error> {
    let found = scope
        .boundaries
        .iter()
        .find(|entry| entry.tag == tag)
        .ok_or_else(|| {
            keel::adapt::Error::Missing(format!("compact {label} boundary {tag} in this strand"))
        })?;
    if !found.fixed {
        return Err(keel::adapt::Error::Adapt(format!(
            "[compact.boundary] compact {label} boundary must be a fixed projected message or a proven completed exchange endpoint"
        )));
    }
    Ok(found)
}
