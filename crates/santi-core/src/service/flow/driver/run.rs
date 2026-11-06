use crate::context::budget::gauged;
use crate::service::flow::failure::{Admission, Failure, Metadata, Operation, Persistence, Stage};
use crate::service::{Service, address::Address, interrupt::Control, notice::Observation, timing};
use santi_provider::Request;

use super::*;
use super::{finish::Finish, invoke::Invocation};
use crate::{message, turn};

impl Service {
    pub(super) async fn run(
        &self,
        strand: &str,
        turn: &str,
        control: &Control,
    ) -> Result<Finish, Failure> {
        let mut prose = String::new();
        let mut last: Option<message::Placed> = None;
        let mut timing = timing::Turn::new(turn);
        let mut round = 0;
        macro_rules! attempt {
            ($operation:expr, $expr:expr) => {
                match $expr {
                    Ok(value) => value,
                    Err(error) => {
                        return Err(Failure::runtime($operation, error, &prose));
                    }
                }
            };
        }

        let provider = match self.recovery(turn).await {
            Ok(provider) => provider,
            Err(error) => {
                return Err(Failure::runtime(Operation::Assembly, error, &prose));
            }
        };
        let actor = match attempt!(
            Operation::Persistence(Persistence::Assistant),
            self.store.strand(strand).await
        ) {
            Some(strand) => strand.soul,
            None => {
                return Err(Failure::runtime(
                    Operation::Persistence(Persistence::Assistant),
                    "strand not found".to_string(),
                    &prose,
                ));
            }
        };
        let response = loop {
            let next = round + 1;
            self.activity.update(
                strand,
                turn::activity::Current {
                    turn: turn.to_string(),
                    round: next,
                    phase: turn::activity::Phase::Preparing,
                    call: None,
                },
            );
            if let Some(error) = attempt!(
                Operation::Admission(Admission::Execution),
                self.readmit(strand, turn, next).await
            ) {
                return Err(Failure::execution(error, &prose));
            }
            round = next;
            if let Some(cause) = self.halted(control) {
                return Err(Failure::stopped(cause, &prose));
            }
            self.noticed(turn).await;
            attempt!(
                Operation::Assembly,
                self.settling(strand, turn, round).await
            );
            if self.settlement(turn).is_none()
                && attempt!(
                    Operation::Assembly,
                    self.pressured(strand, provider.as_ref()).await
                )
            {
                self.press(turn);
            }
            if self.settlement(turn).is_none() {
                let brain = attempt!(Operation::Assembly, self.store.strand(strand).await)
                    .ok_or_else(|| {
                        Failure::runtime(Operation::Assembly, "strand not found".into(), &prose)
                    })?;
                if let crate::service::flow::memory::Gate::Pause { maintainer } = attempt!(
                    Operation::Admission(Admission::Context),
                    self.gate(&brain, provider.as_ref()).await
                ) {
                    let pause = message::Content::text(format!(
                        "<system_message>\nkind: soul_memory_pause\nturn: {turn}\nmaintenance_strand: {maintainer}\nstate: This turn paused because shared memory exceeds its allowance. Recorded history and tool results remain available.\n</system_message>"
                    ));
                    return Ok(Finish {
                        last,
                        response: None,
                        settled: false,
                        pause: Some(pause),
                    });
                }
            }
            let absorbed = if self.eligible(strand, turn, round) {
                attempt!(
                    Operation::Assembly,
                    self.absorb(strand, turn, provider.as_ref(), control).await
                )
            } else {
                Vec::new()
            };
            let input = attempt!(Operation::Assembly, self.assembled(strand, turn).await);
            let metadata = provider.metadata();
            let family = metadata.provider.to_string();
            let offered = attempt!(Operation::Assembly, self.offers(strand, Some(turn)).await);
            let composition = attempt!(Operation::Prompt, self.composition(strand).await);
            let request = Request {
                model: metadata.model,
                instructions: Some(composition.text.clone()),
                input,
                tools: Some(offered),
                previous: None,
            };
            let mut request = attempt!(Operation::Assembly, provider.prepare(request));
            self.suffix(provider.as_ref(), &mut request);
            attempt!(Operation::History, provider.validate(&request));
            let estimate = gauged(&request, provider.as_ref());
            if let Some(error) = attempt!(
                Operation::Admission(Admission::Context),
                self.overdrawn(provider.as_ref(), strand, turn, (&request, &estimate))
                    .await
            ) {
                timing.failed(round, "context_budget", &error.to_string());
                return Err(Failure::context(error));
            }
            timing.built(
                round,
                request.input.len(),
                request.instructions.as_ref().map_or(0, |text| text.len()),
            );
            self.observed(Observation {
                address: Address { strand, turn },
                round,
                provider: &family,
                model: &request.model,
            });
            if let Some(cause) = self.halted(control) {
                return Err(Failure::stopped(cause, &prose));
            }
            attempt!(
                Operation::Assembly,
                composition.capture(&self.store, turn, round).await
            );
            attempt!(
                Operation::Assembly,
                self.witnessed(turn, round, absorbed, &request.input).await
            );
            self.activity.update(
                strand,
                turn::activity::Current {
                    turn: turn.to_string(),
                    round,
                    phase: turn::activity::Phase::Requesting,
                    call: None,
                },
            );
            self.stirred(strand, turn, turn::Motion::Requesting, None);
            let model = request.model.clone();
            let reached = tokio::select! {
                cause = control.wait() => return Err(Failure::stopped(cause, &prose)),
                reached = provider.stream(request) => reached,
            };
            let stream = match reached {
                Ok(stream) => {
                    timing.reached(round);
                    stream
                }
                Err(error) => {
                    timing.failed(round, "http_response", &error);
                    return Err(Failure::provider(
                        error,
                        &prose,
                        Metadata {
                            provider: family.clone(),
                            model: model.clone(),
                            stage: Stage::Request,
                            round,
                        },
                    ));
                }
            };
            let Output {
                calls,
                completed,
                active,
                prose: speech,
            } = Driver {
                service: self,
                address: Address { strand, turn },
                number: round,
                family: &family,
                model: &model,
                prose: &mut prose,
                timing: &mut timing,
                calls: Vec::new(),
                completed: None,
                active: None,
                span: None,
                sketch: None,
                summary: String::new(),
                speech: String::new(),
                seen: false,
                ledger: Default::default(),
            }
            .consume(stream, control)
            .await?;

            if !speech.is_empty() {
                last = Some(attempt!(
                    Operation::Persistence(Persistence::Assistant),
                    self.store
                        .place(santi_estate::MessageDraft {
                            tag: &crate::tag("msg"),
                            strand,
                            actor: crate::message::Role::Soul,
                            actor_id: &actor,
                            kind: crate::message::Kind::Text,
                            content: &crate::message::Content::text(&speech),
                            state: crate::message::State::Fixed,
                            request: false,
                            created: &crate::now(),
                        })
                        .await
                ));
                prose.clear();
            }

            if calls.is_empty() {
                if self.settlement(turn).is_none() {
                    break completed;
                }
                if self.replied(turn) {
                    continue;
                }
                let error = attempt!(
                    Operation::Admission(Admission::Execution),
                    self.incomplete(strand, turn, round).await
                );
                return Err(Failure::execution(error, &prose));
            }

            if self
                .invoke(
                    Invocation {
                        address: Address { strand, turn },
                        round,
                        calls,
                        reply: last.as_ref(),
                        response: completed.as_deref(),
                        active,
                        prose: &prose,
                        timing: &timing,
                    },
                    control,
                )
                .await?
            {
                return Ok(Finish {
                    last,
                    response: completed,
                    settled: true,
                    pause: None,
                });
            }
        };

        Ok(Finish {
            last,
            response,
            settled: false,
            pause: None,
        })
    }
}
