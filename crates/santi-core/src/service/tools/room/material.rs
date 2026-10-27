use crate::service::{
    Service,
    tools::{argued, reply},
};
use crate::{environ::Scope, material};
use santi_provider::{Call, Function, Tool};
use serde::Deserialize;
use serde_json::{Value, json};

pub(in crate::service::tools) fn definition() -> Tool {
    Tool::Function(Function {
        name: "material".into(),
        description: "During maintenance, query loaded material or drop an own binding to reduce fixed context. Soul scope is shared by all its strands. Drop preserves sources, history and learned memory; it affects the next provider round. Loading requires the ordinary santi CLI.".into(),
        parameters: json!({"type":"object","properties":{
            "action":{"type":"string","enum":["query","drop"]},
            "scope":{"type":"string","enum":["soul","strand"]},
            "reference":{"type":"string"}},"required":["action","scope"],"additionalProperties":false}),
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    action: String,
    scope: Scope,
    reference: Option<String>,
}

impl Service {
    async fn remedied(&self, strand: &str, call: &Call) -> Result<Value, String> {
        let args: Args = argued(&call.arguments)?;
        let brain = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let owner = if args.scope == Scope::Soul {
            &brain.soul
        } else {
            strand
        };
        match args.action.as_str() {
            "query" => {
                let selection = self.store.bindings(args.scope, owner).await?;
                Ok(
                    json!({"bindings":selection.bindings,"materials":selection.packages.iter().map(|p| &p.material).collect::<Vec<_>>()}),
                )
            }
            "drop" => {
                let reference = args
                    .reference
                    .ok_or_else(|| "material drop requires reference".to_string())?;
                let applied = self
                    .drop(
                        args.scope,
                        owner,
                        material::Command {
                            reference,
                            operation: Some(call.call.clone()),
                        },
                    )
                    .await?;
                serde_json::to_value(applied).map_err(|e| e.to_string())
            }
            _ => Err("material maintenance supports query and drop".into()),
        }
    }

    pub(in crate::service) async fn remedy(
        &self,
        strand: &str,
        call: &Call,
        limit: Option<usize>,
    ) -> Result<crate::tool::Reply, String> {
        match self.remedied(strand, call).await {
            Ok(value) => {
                self.store
                    .create_reply(santi_estate::ReplyDraft {
                        tag: &crate::tag("result"),
                        call: &call.call,
                        output: Some(&santi_estate::bounded(value, limit)),
                        error: None,
                        created: &crate::now(),
                    })
                    .await
            }
            Err(error) => reply::rejected(self, call, error, limit).await,
        }
    }
}
