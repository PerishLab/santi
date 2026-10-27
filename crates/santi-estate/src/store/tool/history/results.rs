use super::{BATCH, Reader, Rows, Store, decode, keys, need, read};
use keel::Row;
use santi_model::tool;

impl Store {
    pub async fn results(&self, strand: &str) -> Result<Vec<tool::Reply>, String> {
        let reader = Reader(self);
        let entries = reader.entries(strand, "tool_result").await?;
        let mut replies = Vec::with_capacity(entries.len());
        for chunk in entries.chunks(BATCH) {
            let held = keys(chunk.iter(), "result")?;
            let rows = reader.rows("ToolResult", "id", &held).await?;
            let outputs = reader.rows("ToolOutput", "result", &held).await?;
            let failures = reader.rows("ToolFailure", "result", &held).await?;
            let calls = reader
                .rows("ToolCall", "id", &keys(rows.values(), "call")?)
                .await?;
            let outcomes = Outcomes {
                outputs,
                failures,
                calls,
            };
            for entry in chunk {
                let row = need(&rows, "ToolResult", read::int(entry, "result")?)?;
                replies.push(outcomes.decode(row)?);
            }
        }
        Ok(replies)
    }
}

struct Outcomes {
    outputs: Rows,
    failures: Rows,
    calls: Rows,
}

impl Outcomes {
    fn decode(&self, row: &Row) -> Result<tool::Reply, String> {
        let (output, error) =
            decode::outcome(self.outputs.get(&row.key()), self.failures.get(&row.key()))?;
        Ok(tool::Reply {
            id: read::text(row, "tag")?.to_string(),
            call: read::text(
                need(&self.calls, "ToolCall", read::int(row, "call")?)?,
                "tag",
            )?
            .to_string(),
            output,
            error,
            created: read::text(row, "created")?.to_string(),
        })
    }
}
