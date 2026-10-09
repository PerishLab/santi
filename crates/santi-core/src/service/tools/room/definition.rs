use santi_provider::{Function, Tool};
use serde_json::json;

pub(in crate::service::tools) fn definition() -> Tool {
    Tool::Function(Function {
        name: "compact".to_string(),
        description: "Summarize settled strand history to restore room for ordinary tools. Supply summary alone to compact the settled history since the last occupied slot, keeping the most recent message live. Use absorb to rewrite one occupied slot or merge contiguous slots; merging two or more frees a slot. Shorter summaries reduce compacted bytes. Use first and last for a precise message range. Originals remain queryable. This tool is available during context maintenance.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "The summary that replaces the collapsed range."
                },
                "absorb": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Occupied Compact IDs from compact projection headers. One rewrites its summary; two or more contiguous slots merge into one."
                },
                "first": {
                    "type": "string",
                    "description": "First Message ID from a [message ID] header or compact covered_message_range. Supply with last."
                },
                "last": {
                    "type": "string",
                    "description": "Last Message ID from a [message ID] header or compact covered_message_range. Supply with first."
                }
            },
            "required": ["summary"],
            "additionalProperties": false
        }),
    })
}
