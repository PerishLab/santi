use santi_provider::{Function, Tool};
use serde_json::json;

use crate::{SOULSPACE, STRANDSPACE, soulward, strandward};

pub(super) fn tool() -> Tool {
    let soulward = soulward();
    let strandward = strandward();
    Tool::Function(Function {
        name: "shell".to_string(),
        description: format!(
            "Run a short, bounded shell command. If completion time is unknown, the command waits on external state, or periodic attention is useful, use this invocation only to call `santi job create <DESCRIPTION> <COMMAND>`; optionally add `--cwd`, `--timeout-seconds`, `--output-limit-bytes`, or `--remind-every-seconds`. Santi supplies the one-use capability, creates a durable detached job with timeout and output bounds, and returns its id. Never keep this synchronous shell open merely to wait or poll. By default commands run in the current execution workspace. Use cwd \"{SOULSPACE}\" to work in the current soul workspace, where {soulward} is always rendered live in [santi-soul]. Use cwd \"{STRANDSPACE}\" to work in the current strand workspace, where {strandward} is always rendered live in [santi-strand]. Unix-like systems use bash by default; Windows uses pwsh by default."
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
                    "description": format!("Optional workspace URI. Supports {SOULSPACE}, {SOULSPACE}<path>, {STRANDSPACE}, and {STRANDSPACE}<path>.")
                }
            },
            "required": ["command"],
            "additionalProperties": false
        }),
    })
}
