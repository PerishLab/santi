use crate::service::flow::failure::{Failure, Operation, Persistence};
use crate::service::{Service, address::Address, interrupt::Control};
use crate::{message, stream};
use santi_provider::Call;

pub(super) struct Maintenance<'a> {
    pub(super) address: Address<&'a str>,
    pub(super) round: usize,
    pub(super) calls: Vec<Call>,
    pub(super) limits: Vec<Option<usize>>,
    pub(super) reply: Option<&'a message::Placed>,
    pub(super) response: Option<&'a str>,
}

impl Service {
    pub(super) async fn maintained(
        &self,
        draft: Maintenance<'_>,
        control: &Control,
    ) -> Result<bool, Failure> {
        let Address { strand, turn } = draft.address;
        if let Some(cause) = self.halted(control) {
            return Err(Failure::stopped(cause, ""));
        }
        let calls = self
            .staged(draft.address, draft.calls, draft.limits)
            .map_err(|error| Failure::runtime(Operation::Tool, error, ""))?;
        if let Some(cause) = self.halted(control) {
            return Err(Failure::stopped(cause, ""));
        }
        let policy = self.regimen();
        let reason = self
            .reason(turn)
            .unwrap_or_else(|| "execution_budget".into());
        let pause = format!(
            "<system_message>\nkind: execution_pause\nturn: {turn}\nreason: {reason}\nprovider_rounds: {}\nstate: This turn paused after a successful compact. Recorded history and tool results remain available.\n</system_message>",
            draft.round
        );
        let content = message::Content::text(&pause);
        let source = crate::ingest::Source::new("execution_pause").with_ref(turn.to_string());
        let metadata = self.provider.metadata();
        let settlement = self
            .store
            .maintain(santi_estate::Maintenance {
                calls: &calls,
                limits: santi_estate::Limits {
                    slots: policy.slots,
                    settled: policy.settled,
                },
                completion: santi_estate::CompletionDraft {
                    turn,
                    reply: draft.reply.map(|message| message.message.id.as_str()),
                    provider: &metadata.provider,
                    model: &metadata.model,
                    response: draft.response,
                    occurred: &crate::now(),
                },
                inbox: santi_estate::InboxDraft {
                    tag: &crate::tag("inbox"),
                    strand,
                    kind: message::Kind::SantiSystem,
                    content: &content,
                    source: Some(&source),
                    created: &crate::now(),
                },
            })
            .await;
        let settlement = match settlement {
            Ok(settlement) => settlement,
            Err(error) => return Err(self.unsettled(turn, error).await),
        };
        for (call, result) in settlement.calls.into_iter().zip(settlement.replies) {
            self.publish(
                strand,
                stream::Payload::Tool(crate::tool::Beat::Called { call }),
            );
            self.publish(
                strand,
                stream::Payload::Tool(crate::tool::Beat::Replied { result }),
            );
        }
        if !settlement.completed {
            return Ok(false);
        }
        if let Some(message) = draft.reply {
            self.publish(
                strand,
                stream::Payload::Message(crate::message::Beat::Completed {
                    turn: turn.into(),
                    message: message.clone(),
                }),
            );
        }
        if let Err(error) = self.absolve(strand, "compact_exec").await {
            eprintln!("santi: committed maintenance resolution failed: {error}");
        }
        self.dispatched().await;
        self.publish(
            strand,
            stream::Payload::Turn(crate::turn::Beat::Completed {
                turn: turn.into(),
                label: settlement.event.map(|event| event.label),
                text: Some(pause),
            }),
        );
        Ok(true)
    }

    async fn unsettled(&self, turn: &str, error: String) -> Failure {
        match self.store.stop(turn).await {
            Ok(Some(stop)) if stop.cause.is_some() => Failure::stopped(stop.cause.unwrap(), ""),
            Ok(_) | Err(_) => {
                Failure::runtime(Operation::Persistence(Persistence::Completion), error, "")
            }
        }
    }
}
