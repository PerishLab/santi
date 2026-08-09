use keel::{Op, form};

use crate::store::Store;
use crate::store::support::read;

const BATCH: usize = 400;

pub enum Spent {
    Output(serde_json::Value),
    Error(String),
}

pub struct Tally {
    pub calls: usize,
    pub spent: Vec<Spent>,
}

impl Store {
    pub async fn tally(&self, strand: &str) -> Result<Tally, String> {
        let strand = read::one(&self.core, "Strand", "tag", strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let key = strand.key().to_string();
        let calls = self
            .core
            .ask(&form("StrandEntry").when("strand", Op::Eq, &key).when(
                "target_type",
                Op::Eq,
                "tool_call",
            ))
            .await
            .map_err(read::error)?
            .rows()
            .len();
        let entries = self
            .core
            .ask(&form("StrandEntry").when("strand", Op::Eq, &key).when(
                "target_type",
                Op::Eq,
                "tool_result",
            ))
            .await
            .map_err(read::error)?;
        let mut tags = Vec::with_capacity(entries.rows().len());
        for entry in entries.rows() {
            tags.push(read::text(entry, "target")?.to_string());
        }
        let mut spent = Vec::with_capacity(tags.len());
        for batch in tags.chunks(BATCH) {
            let held = batch.iter().map(String::as_str).collect::<Vec<_>>();
            let rows = self
                .core
                .ask(&form("ToolResult").any("tag", &held))
                .await
                .map_err(read::error)?;
            let mut keys = Vec::with_capacity(rows.rows().len());
            for row in rows.rows() {
                keys.push(row.key().to_string());
            }
            let outer = keys.iter().map(String::as_str).collect::<Vec<_>>();
            let outputs = self
                .core
                .ask(&form("ToolOutput").any("result", &outer))
                .await
                .map_err(read::error)?;
            for row in outputs.rows() {
                spent.push(Spent::Output(
                    serde_json::from_str(read::text(row, "output")?)
                        .map_err(|error| error.to_string())?,
                ));
            }
            let failures = self
                .core
                .ask(&form("ToolFailure").any("result", &outer))
                .await
                .map_err(read::error)?;
            for row in failures.rows() {
                spent.push(Spent::Error(read::text(row, "error")?.to_string()));
            }
        }
        Ok(Tally { calls, spent })
    }
}
