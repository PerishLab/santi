use santi_provider::{Call, Tool};
use serde::Deserialize;
use serde_json::Value;

use crate::capability::Origin;
use crate::service::address::Address;

use super::Service;
use crate::service::interrupt::Control;
use crate::{effect, stream};

mod clock;
mod environ;
mod feedback;
mod reply;
mod shell;
mod wake;
mod workspace;

pub(crate) fn tools() -> Vec<Tool> {
    vec![shell::definition()]
}

struct Shell<'a> {
    strand: &'a str,
    turn: &'a str,
    call: &'a Call,
    effect: &'a str,
    limit: Option<usize>,
    fixed: Option<shell::Args>,
}

impl Service {
    pub(super) async fn tooled(
        &self,
        address: Address<&str>,
        call: Call,
        output_limit: Option<usize>,
        control: &Control,
    ) -> Result<(), String> {
        let Address { strand, turn } = address;
        let clock = clock::selected(self, strand).await?;
        let barrier = if clock {
            None
        } else {
            self.barrier(strand).await?
        };
        let barred = call.name == "shell" && barrier.as_ref().is_some_and(feedback::Barrier::due);
        let due = call.name == "feedback" && barrier.as_ref().is_some_and(feedback::Barrier::due);
        let fixed = due
            .then(|| barrier.as_ref().and_then(feedback::Barrier::owned))
            .flatten();
        let allowed = !clock || call.name == "wake";
        let shell = call.name == "shell" || due;
        let kind = (allowed && !barred && shell).then_some("shell");
        let created = crate::now();
        let effect_tag = kind.map(|_| crate::tag("effect"));
        let (held, effect) = self
            .store
            .prepare_invocation(
                santi_estate::CallDraft {
                    tag: &call.call,
                    turn,
                    tool: &call.name,
                    arguments: &call.arguments,
                    created: &created,
                },
                effect_tag.as_deref().map(|tag| santi_estate::EffectDraft {
                    tag,
                    turn,
                    call: Some(&call.call),
                    kind: kind.expect("effect kind"),
                    metadata: None,
                    created: &created,
                }),
            )
            .await?;
        self.publish(
            strand,
            stream::Payload::Tool(crate::tool::Beat::Called { call: held.clone() }),
        );
        let result = if !allowed {
            reply::rejected(
                self,
                &call,
                format!("unsupported tool on clock attention strand: {}", call.name),
                output_limit,
            )
            .await?
        } else if barred {
            let barrier = barrier.expect("barred shell has feedback barrier");
            let action = if barrier.command.is_some() {
                "call the zero-argument caller-owned feedback tool, or reply BLOCKED without a tool"
            } else {
                "call feedback with kind native or slice, or reply BLOCKED without a tool"
            };
            let error = format!(
                "feedback barrier reached after {} ordinary shell calls (limit {}); {action}",
                barrier.observed, barrier.limit
            );
            reply::rejected(self, &call, error, output_limit).await?
        } else if let Some(effect) = effect {
            self.shelled(
                Shell {
                    strand,
                    turn,
                    call: &call,
                    effect: &effect.id,
                    limit: output_limit,
                    fixed,
                },
                control,
            )
            .await?
        } else if call.name == "wake" {
            self.waked(strand, &call, output_limit).await?
        } else {
            reply::rejected(
                self,
                &call,
                format!("unsupported tool: {}", call.name),
                output_limit,
            )
            .await?
        };
        self.publish(
            strand,
            stream::Payload::Tool(crate::tool::Beat::Replied { result }),
        );
        Ok(())
    }

    async fn shelled(
        &self,
        shell: Shell<'_>,
        control: &Control,
    ) -> Result<crate::tool::Reply, String> {
        let Shell {
            strand,
            turn,
            call,
            effect,
            limit: output_limit,
            fixed,
        } = shell;
        let soul = self
            .store
            .strand(strand)
            .await?
            .map(|strand| strand.soul)
            .ok_or_else(|| "strand not found".to_string())?;
        let args = match call.name.as_str() {
            "feedback" => feedback::args(call, fixed),
            _ => argued::<shell::Args>(&call.arguments),
        };
        let preparation = match args {
            Ok(args) => {
                self.prepared(
                    Origin {
                        strand,
                        turn,
                        soul: &soul,
                        call: &call.call,
                        effect,
                    },
                    args,
                )
                .await
            }
            Err(error) => Err(error),
        };
        let prepared = match preparation {
            Ok(prepared) => prepared,
            Err(error) => {
                let error = curbed(error, output_limit);
                return self
                    .store
                    .redeem_effect(
                        effect,
                        santi_estate::RedemptionDraft {
                            result: &crate::tag("result"),
                            call: &call.call,
                            output: None,
                            error: Some(&error),
                            outcome: effect::Outcome::NotApplied,
                            occurred: &crate::now(),
                        },
                    )
                    .await;
            }
        };
        if let Some(cause) = self.halted(control) {
            let error = format!("interrupted by {} before dispatch", cause.encode());
            return self
                .store
                .redeem_effect(
                    effect,
                    santi_estate::RedemptionDraft {
                        result: &crate::tag("result"),
                        call: &call.call,
                        output: None,
                        error: Some(&error),
                        outcome: effect::Outcome::NotApplied,
                        occurred: &crate::now(),
                    },
                )
                .await;
        }
        self.store.dispatch_effect(effect, &crate::now()).await?;
        match shell::ran(prepared, output_limit, control).await {
            shell::Outcome::Captured(output) => {
                self.store
                    .redeem_effect(
                        effect,
                        santi_estate::RedemptionDraft {
                            result: &crate::tag("result"),
                            call: &call.call,
                            output: Some(&output),
                            error: None,
                            outcome: effect::Outcome::Applied,
                            occurred: &crate::now(),
                        },
                    )
                    .await
            }
            shell::Outcome::Failed(error) => {
                let error = curbed(error, output_limit);
                self.store
                    .redeem_effect(
                        effect,
                        santi_estate::RedemptionDraft {
                            result: &crate::tag("result"),
                            call: &call.call,
                            output: None,
                            error: Some(&error),
                            outcome: effect::Outcome::NotApplied,
                            occurred: &crate::now(),
                        },
                    )
                    .await
            }
            shell::Outcome::Unknown(error) => {
                self.store
                    .unknown_effect(effect, &error, &crate::now())
                    .await?;
                Err(format!(
                    "shell effect {effect} outcome is unknown; automatic replay is forbidden: {error}"
                ))
            }
            shell::Outcome::Stopped(error) => {
                self.store
                    .unknown_effect(effect, &error, &crate::now())
                    .await?;
                Err(error)
            }
        }
    }
}

pub(super) fn curbed(error: String, limit: Option<usize>) -> String {
    let Some(limit) = limit else {
        return error;
    };
    if error.len() <= limit {
        return error;
    }
    let mut end = limit;
    while end > 0 && !error.is_char_boundary(end) {
        end -= 1;
    }
    error[..end].to_string()
}

pub(super) fn argued<T: for<'de> Deserialize<'de>>(value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|error| error.to_string())
}
