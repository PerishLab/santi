use santi_model::tool;

use crate::store::Store;
use crate::store::support::read;

impl Store {
    pub(super) async fn decode_call(&self, row: &keel::Row) -> Result<tool::Call, String> {
        Ok(tool::Call {
            id: read::text(row, "tag")?.to_string(),
            turn: read::related(&self.core, "Turn", read::int(row, "turn")?).await?,
            tool: read::text(row, "tool")?.to_string(),
            arguments: serde_json::from_str(read::text(row, "arguments")?)
                .map_err(|error| error.to_string())?,
            created: read::text(row, "created")?.to_string(),
        })
    }
    pub(super) async fn decode_reply(&self, row: &keel::Row) -> Result<tool::Reply, String> {
        let key = row.key().to_string();
        let output = read::one(&self.core, "ToolOutput", "result", &key).await?;
        let failure = read::one(&self.core, "ToolFailure", "result", &key).await?;
        let (output, error) = match (output, failure) {
            (Some(output), None) => (
                Some(
                    serde_json::from_str(read::text(&output, "output")?)
                        .map_err(|error| error.to_string())?,
                ),
                None,
            ),
            (None, Some(failure)) => (None, Some(read::text(&failure, "error")?.to_string())),
            (None, None) => return Err("tool result has no outcome".to_string()),
            (Some(_), Some(_)) => return Err("tool result has conflicting outcomes".to_string()),
        };
        Ok(tool::Reply {
            id: read::text(row, "tag")?.to_string(),
            call: read::related(&self.core, "ToolCall", read::int(row, "call")?).await?,
            output,
            error,
            created: read::text(row, "created")?.to_string(),
        })
    }
}
