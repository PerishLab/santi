use super::{BATCH, Reader, Rows, Store, keys, need, read};
use keel::Row;
use santi_model::tool;

impl Store {
    pub async fn calls(&self, strand: &str) -> Result<Vec<tool::Call>, String> {
        let reader = Reader(self);
        let entries = reader.entries(strand, "tool_call").await?;
        let mut calls = Vec::with_capacity(entries.len());
        for chunk in entries.chunks(BATCH) {
            let rows = reader
                .rows("ToolCall", "id", &keys(chunk.iter(), "call")?)
                .await?;
            let turns = reader
                .rows("Turn", "id", &keys(rows.values(), "turn")?)
                .await?;
            for entry in chunk {
                let row = need(&rows, "ToolCall", read::int(entry, "call")?)?;
                calls.push(decode(row, &turns)?);
            }
        }
        Ok(calls)
    }
}

fn decode(row: &Row, turns: &Rows) -> Result<tool::Call, String> {
    Ok(tool::Call {
        id: read::text(row, "tag")?.to_string(),
        turn: read::text(need(turns, "Turn", read::int(row, "turn")?)?, "tag")?.to_string(),
        tool: read::text(row, "tool")?.to_string(),
        arguments: serde_json::from_str(read::text(row, "arguments")?)
            .map_err(|error| error.to_string())?,
        created: read::text(row, "created")?.to_string(),
    })
}
