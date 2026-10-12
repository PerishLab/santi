use santi_provider::{Function, Tool};
use serde_json::json;

use crate::{SOULSPACE, STRANDSPACE};

pub(super) fn tool() -> Tool {
    Tool::Function(Function {
        name: "shell".to_string(),
        description: format!(
            "Run a short, bounded command in a fresh shell process. For work with unknown duration, external waits, or periodic attention, use this invocation solely to create one detached Job with `santi job create <DESCRIPTION> <COMMAND>`; its help lists options. Santi supplies the one-use capability and returns the durable Job id. Commands default to the current execution workspace; cwd selects {SOULSPACE} or {STRANDSPACE} and their subpaths. Unix-like systems use bash; Windows uses pwsh."
        ),
        parameters: json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute."
                },
                "cwd": {
                    "type": "string",
                    "description": format!("Omit or leave empty for the current execution workspace. Select {SOULSPACE}, {SOULSPACE}<path>, {STRANDSPACE}, or {STRANDSPACE}<path> for a workspace URI.")
                }
            },
            "required": ["command"],
            "additionalProperties": false
        }),
    })
}
