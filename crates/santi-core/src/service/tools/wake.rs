use santi_provider::{Call, Function, Tool};
use serde::Deserialize;
use serde_json::json;

use super::{Service, argued, curbed};

#[derive(Deserialize)]
pub(super) struct Args {
    pub action: Action,
    pub generation: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    Status,
    Renew,
    Silence,
}

pub(super) fn definition() -> Tool {
    Tool::Function(Function {
        name: "wake".to_string(),
        description: "Inspect, explicitly renew, or silence this soul's caller-permitted autonomous wake lease. A renewal resets the allowance to three future wake rounds. Use the generation reported by the clock message or status action. Ordinary work never renews a lease implicitly, and a caller revocation cannot be overridden.".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["status", "renew", "silence"],
                    "description": "Inspect the lease, renew it, or silence it."
                },
                "generation": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Required for renew and silence; copy the current lease generation exactly."
                }
            },
            "required": ["action"],
            "additionalProperties": false
        }),
    })
}

impl Service {
    pub(super) async fn waked(
        &self,
        strand: &str,
        call: &Call,
        output_limit: Option<usize>,
    ) -> Result<crate::tool::Reply, String> {
        let soul = self
            .store
            .strand(strand)
            .await?
            .map(|strand| strand.soul)
            .ok_or_else(|| "strand not found".to_string())?;
        let result = match argued::<Args>(&call.arguments) {
            Ok(args) => match args.action {
                Action::Status => self
                    .wake(&soul)
                    .await?
                    .ok_or_else(|| "soul not found".to_string()),
                Action::Renew => match args.generation {
                    Some(generation) => self.renew_wake(&soul, generation).await,
                    None => Err("wake renew requires generation".to_string()),
                },
                Action::Silence => match args.generation {
                    Some(generation) => self.silence_wake(&soul, generation).await,
                    None => Err("wake silence requires generation".to_string()),
                },
            },
            Err(error) => Err(error),
        };
        let tag = crate::tag("result");
        match result {
            Ok(lease) => {
                let output = serde_json::to_value(lease).map_err(|error| error.to_string())?;
                self.store
                    .create_reply(santi_estate::ReplyDraft {
                        tag: &tag,
                        call: &call.call,
                        output: Some(&output),
                        error: None,
                        created: &crate::now(),
                    })
                    .await
            }
            Err(error) => {
                let error = curbed(error, output_limit);
                self.store
                    .create_reply(santi_estate::ReplyDraft {
                        tag: &tag,
                        call: &call.call,
                        output: None,
                        error: Some(&error),
                        created: &crate::now(),
                    })
                    .await
            }
        }
    }
}
