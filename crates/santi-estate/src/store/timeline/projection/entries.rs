use super::{Store, read, target};
use crate::store::entry;
use keel::{Op, Rank, Row, form};
use santi_model::strand;
use std::collections::{BTreeMap, BTreeSet};

const BATCH: usize = 400;
type Targets = BTreeMap<String, BTreeMap<i64, String>>;
struct Reader<'a>(&'a Store);

impl Store {
    pub async fn entries(&self, strand: &str) -> Result<Vec<strand::Entry>, String> {
        let root = read::one(&self.core, "Strand", "tag", strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let rows = self
            .core
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &root.key().to_string())
                    .order("sequence", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        let owner = read::text(&root, "tag")?;
        let mut entries = Vec::with_capacity(rows.rows().len());
        for chunk in rows.rows().chunks(BATCH) {
            let targets = Reader(self).targets(chunk).await?;
            for row in chunk {
                entries.push(project(owner, row, &targets)?);
            }
        }
        Ok(entries)
    }
}

impl Reader<'_> {
    async fn targets(&self, rows: &[Row]) -> Result<Targets, String> {
        let mut targets = Targets::new();
        for (unit, keys) in requests(rows)? {
            let held = keys.iter().map(String::as_str).collect::<Vec<_>>();
            let pack = self
                .0
                .core
                .ask(&form(&unit).any("id", &held))
                .await
                .map_err(read::error)?;
            let tags = pack
                .rows()
                .iter()
                .map(|row| Ok((row.key(), read::text(row, "tag")?.to_string())))
                .collect::<Result<_, String>>()?;
            targets.insert(unit, tags);
        }
        Ok(targets)
    }
}

fn requests(rows: &[Row]) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut requests = BTreeMap::<String, BTreeSet<String>>::new();
    for row in rows {
        let kind = read::text(row, "target_type")?;
        target::decode(kind)?;
        let unit = entry::unit(kind).map_err(|error| error.to_string())?;
        let field = entry::column(kind).map_err(|error| error.to_string())?;
        requests
            .entry(unit.into())
            .or_default()
            .insert(read::int(row, field)?.to_string());
    }
    Ok(requests)
}

fn project(owner: &str, row: &Row, targets: &Targets) -> Result<strand::Entry, String> {
    let kind = read::text(row, "target_type")?;
    let unit = entry::unit(kind).map_err(|error| error.to_string())?;
    let field = entry::column(kind).map_err(|error| error.to_string())?;
    let key = read::int(row, field)?;
    let tag = targets
        .get(unit)
        .and_then(|tags| tags.get(&key))
        .ok_or_else(|| format!("{unit} {key} missing"))?;
    Ok(strand::Entry {
        strand: owner.to_string(),
        kind: target::decode(kind)?,
        target: tag.clone(),
        seq: read::int(row, "sequence")?,
        created: read::text(row, "created")?.to_string(),
    })
}
