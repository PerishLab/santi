use santi_provider::{Call, Function, Tool};
use serde::Deserialize;
use std::collections::HashSet;

use super::{Service, argued, shell, tools, wake};
use crate::effect;

const OWNED: &str = r#"{
    "type": "object",
    "properties": {},
    "additionalProperties": false
}"#;

const SELECTED: &str = r#"{
    "type": "object",
    "properties": {
        "kind": {
            "type": "string",
            "enum": ["native", "slice"],
            "description": "The feedback action this command performs."
        },
        "command": {
            "type": "string",
            "description": "One bounded shell command that obtains native feedback or makes and checks a minimal product slice."
        },
        "cwd": {
            "type": "string",
            "description": "Optional workspace URI. Supports soul://, soul://<path>, strand://, and strand://<path>."
        }
    },
    "required": ["kind", "command"],
    "additionalProperties": false
}"#;

pub(super) fn definition(limit: usize, owned: bool) -> Tool {
    let description = if owned {
        format!(
            "The feedback barrier is due after {limit} ordinary shell calls. Invoke this zero-argument tool exactly once to run the caller-owned feedback effect. Its command and workspace are fixed outside the model's authority. A captured red or green result is feedback and reopens ordinary shell access."
        )
    } else {
        format!(
            "The opt-in feedback barrier is due after {limit} ordinary shell calls. Run exactly one feedback-bearing command: `native` executes the strongest available compile/test/act ceremony; `slice` makes the smallest reversible product change and immediately compiles or tests it. Do not use this tool for another read-only repository tour. If neither action is justified, call no tool and report a precise BLOCKED result. An applied feedback call reopens ordinary shell access."
        )
    };
    let schema = if owned { OWNED } else { SELECTED };
    Tool::Function(Function {
        name: "feedback".to_string(),
        description,
        parameters: serde_json::from_str(schema).expect("static feedback tool schema"),
    })
}

pub(super) struct Barrier {
    pub(super) limit: usize,
    pub(super) observed: usize,
    pub(super) command: Option<String>,
    cwd: Option<String>,
}

impl Barrier {
    pub(super) fn due(&self) -> bool {
        self.observed >= self.limit
    }

    pub(super) fn owned(&self) -> Option<shell::Args> {
        self.command.as_ref().map(|command| shell::Args {
            command: command.clone(),
            cwd: self.cwd.clone(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum Kind {
    Native,
    Slice,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    kind: Kind,
    command: String,
    cwd: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixed {}

impl Args {
    fn shell(self) -> shell::Args {
        let Self { kind, command, cwd } = self;
        let _ = kind;
        shell::Args { command, cwd }
    }
}

pub(super) fn args(call: &Call, fixed: Option<shell::Args>) -> Result<shell::Args, String> {
    match fixed {
        Some(args) => argued::<Fixed>(&call.arguments).map(|_| args),
        None => argued::<Args>(&call.arguments).map(Args::shell),
    }
}

impl Service {
    pub(in crate::service) async fn offered(&self, strand: &str) -> Result<Vec<Tool>, String> {
        let turn = self.store.latest(strand).await?;
        let owner = turn
            .as_ref()
            .filter(|turn| turn.status == crate::turn::Status::Running)
            .map(|turn| turn.id.as_str());
        self.offers(strand, owner).await
    }

    pub(in crate::service) async fn offers(
        &self,
        strand: &str,
        turn: Option<&str>,
    ) -> Result<Vec<Tool>, String> {
        if super::room::clock::selected(self, strand).await? {
            return Ok(vec![wake::definition()]);
        }
        if turn.and_then(|turn| self.settlement(turn)).is_some()
            || self.crowded(strand).await?.is_some()
        {
            return Ok(vec![super::room::definition()]);
        }
        match self.barrier(strand).await? {
            Some(barrier) if barrier.due() => Ok(vec![
                definition(barrier.limit, barrier.command.is_some()),
                wake::definition(),
            ]),
            _ => Ok(tools()),
        }
    }

    pub(super) async fn barrier(&self, strand: &str) -> Result<Option<Barrier>, String> {
        let Some(budget) = self.rationed(strand) else {
            return Ok(None);
        };
        let Some(limit) = budget.feedback_after_calls else {
            return Ok(None);
        };
        let calls = self.store.calls(strand).await?;
        let applied = self
            .store
            .effects(strand)
            .await?
            .into_iter()
            .filter(|held| matches!(held.state, effect::State::Settled(effect::Outcome::Applied)))
            .filter_map(|held| held.call)
            .collect::<HashSet<_>>();
        let reset = calls
            .iter()
            .rposition(|call| call.tool == "feedback" && applied.contains(&call.id));
        let observed = calls[reset.map_or(0, |index| index + 1)..]
            .iter()
            .filter(|call| call.tool == "shell")
            .count();
        Ok(Some(Barrier {
            limit,
            observed,
            command: budget.feedback_command,
            cwd: budget.feedback_cwd,
        }))
    }
}
