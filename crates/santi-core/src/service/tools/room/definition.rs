use santi_provider::{Function, Tool};
use serde_json::json;

pub(in crate::service::tools) fn definition() -> Tool {
    Tool::Function(Function {
        name: "compact".to_string(),
        description: "Collapse a settled range of this strand into one summary, freeing the room ordinary work needs. With no range, everything settled since the last occupied slot is collapsed and the most recent message stays live. When every slot is occupied, name a range spanning two or more of them so this compaction absorbs them into one; what survives and how the surviving summary reads is yours to decide. This tool is offered only while the context ceiling refuses ordinary work.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "The summary that replaces the collapsed range."
                },
                "first": {
                    "type": "string",
                    "description": "Optional first message id of the range. Give with last."
                },
                "last": {
                    "type": "string",
                    "description": "Optional last message id of the range. Give with first."
                }
            },
            "required": ["summary"],
            "additionalProperties": false
        }),
    })
}
