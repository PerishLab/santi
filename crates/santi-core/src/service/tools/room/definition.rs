use santi_provider::{Function, Tool};
use serde_json::json;

pub(in crate::service::tools) fn definition() -> Tool {
    Tool::Function(Function {
        name: "compact".to_string(),
        description: "Summarize settled strand history to restore room for ordinary tools. Supply summary alone to compact the settled history since the last occupied slot, keeping the most recent message live. Use absorb to rewrite one occupied slot or merge contiguous slots; merging two or more frees a slot. Shorter summaries reduce compacted bytes. Use first and last for a precise message range; with absorb, the compact covers the named slots and the range together. A boundary on a message that cannot anchor a compact, such as an aborted reply, moves inward to the nearest one that can, and the result reports the requested range. A refusal names what blocked the request and, where one exists, arguments ready to submit. Originals remain queryable. This tool is available during context maintenance.".to_string(),
        parameters: parameters(),
    })
}

fn parameters() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "summary": {
                "type": "string",
                "description": "The summary that replaces the collapsed range."
            },
            "absorb": absorb(),
            "first": {
                "type": "string",
                "description": "First Message ID from an inbound [message ID] header or compact covered_message_range. Supply with last."
            },
            "last": {
                "type": "string",
                "description": "Last Message ID from an inbound [message ID] header or compact covered_message_range. Supply with first."
            }
        },
        "required": ["summary"],
        "additionalProperties": false
    })
}

fn absorb() -> serde_json::Value {
    json!({
        "type": "array",
        "items": { "type": "string" },
        "description": "Occupied Compact IDs from compact projection headers. One rewrites its summary; two or more contiguous slots merge into one."
    })
}
