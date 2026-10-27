use super::{Store, decode, read};
use keel::{Op, Rank, Row, form};
use std::collections::{BTreeMap, BTreeSet};

mod calls;
mod results;

const BATCH: usize = 400;
type Rows = BTreeMap<i64, Row>;
struct Reader<'a>(&'a Store);

impl Reader<'_> {
    async fn entries(&self, strand: &str, kind: &str) -> Result<Vec<Row>, String> {
        let strand = read::one(&self.0.core, "Strand", "tag", strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        self.0
            .core
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &strand.key().to_string())
                    .when("target_type", Op::Eq, kind)
                    .order("sequence", Rank::Asc),
            )
            .await
            .map(|pack| pack.rows().to_vec())
            .map_err(read::error)
    }

    async fn rows(&self, unit: &str, field: &str, keys: &[i64]) -> Result<Rows, String> {
        let keys = keys.iter().map(i64::to_string).collect::<BTreeSet<_>>();
        if keys.is_empty() {
            return Ok(Rows::new());
        }
        let held = keys.iter().map(String::as_str).collect::<Vec<_>>();
        let pack = self
            .0
            .core
            .ask(&form(unit).any(field, &held))
            .await
            .map_err(read::error)?;
        pack.rows()
            .iter()
            .map(|row| {
                let key = if field == "id" {
                    row.key()
                } else {
                    read::int(row, field)?
                };
                Ok((key, row.clone()))
            })
            .collect()
    }
}

fn keys<'a>(rows: impl Iterator<Item = &'a Row>, field: &str) -> Result<Vec<i64>, String> {
    rows.map(|row| read::int(row, field)).collect()
}

fn need<'a>(rows: &'a Rows, unit: &str, key: i64) -> Result<&'a Row, String> {
    rows.get(&key)
        .ok_or_else(|| format!("{unit} {key} missing"))
}
