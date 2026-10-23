use crate::context::budget::gauged;
use crate::service::flow::budget::Verdict;
use crate::service::flow::failure::{Admission, Failure, Metadata, Operation, Persistence, Stage};
use crate::service::{Service, address::Address, interrupt::Control, notice::Observation, timing};
use santi_provider::Request;
use std::{future::Future, pin::Pin};

use super::finish::Finish;
use super::maintenance::Maintenance;
use super::*;
use crate::{message, turn};

impl Service {
    pub(in crate::service::flow) fn conduct(
        &self,
        strand: String,
        turn: String,
        control: Control,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(async move {
            match self.run(&strand, &turn, &control).await {
                Err(failure) => self.bury(&strand, &turn, failure).await,
                Ok(finish) if finish.settled => {}
                Ok(finish) => match self.halted(&control) {
                    Some(cause) => self.bury(&strand, &turn, Failure::stopped(cause, "")).await,
                    None => self.land(&strand, &turn, finish).await,
                },
            }
            self.noticed(&turn).await;
            self.release(&turn);
            self.unsettle(&turn);
            self.poke(&strand, "strand_send", None, "turn_completion_poke")
                .await;
            self.relieve(&strand).await;
        })
    }

    async fn run(&self, strand: &str, turn: &str, control: &Control) -> Result<Finish, Failure> {
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
                && attempt!(Operation::Assembly, self.pressured(strand).await)
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
                    self.gate(&brain).await
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
            let input = attempt!(Operation::Assembly, self.assembled(strand, turn).await);
            let metadata = self.provider.metadata();
            let family = metadata.provider.to_string();
            let offered = attempt!(Operation::Assembly, self.offers(strand, Some(turn)).await);
            let mut request = Request {
                model: metadata.model,
                instructions: Some(attempt!(Operation::Prompt, self.wording(strand).await)),
                input,
                tools: Some(offered),
                previous: None,
            };
            self.suffix(&mut request);
            let estimate = gauged(&request);
            if let Some(error) = attempt!(
                Operation::Admission(Admission::Context),
                self.overdrawn(strand, turn, &request, &estimate).await
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
            self.stirred(strand, turn, turn::Motion::Requesting, None);
            let model = request.model.clone();
            let reached = tokio::select! {
                cause = control.wait() => return Err(Failure::stopped(cause, &prose)),
                reached = self.provider.stream(request) => reached,
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

            let limits = match attempt!(
                Operation::Admission(Admission::Execution),
                self.judge(strand, turn, round, calls.len()).await
            ) {
                Verdict::Maintain => continue,
                Verdict::Unbounded => vec![None; calls.len()],
                Verdict::Bounded(limits) => limits.into_iter().map(Some).collect::<Vec<_>>(),
                Verdict::Rejected(error) => {
                    return Err(Failure::execution(*error, &prose));
                }
            };
            if attempt!(Operation::Assembly, self.maintaining(turn).await) {
                timing.outputting(round, calls.len());
                let count = calls.len();
                self.stirred(strand, turn, turn::Motion::Running, active.clone());
                let settled = self
                    .maintained(
                        Maintenance {
                            address: Address { strand, turn },
                            round,
                            calls,
                            limits,
                            reply: last.as_ref(),
                            response: completed.as_deref(),
                        },
                        control,
                    )
                    .await?;
                timing.outputted(round, count);
                if settled {
                    return Ok(Finish {
                        last,
                        response: completed,
                        settled: true,
                        pause: None,
                    });
                }
                continue;
            }
            timing.outputting(round, calls.len());
            let count = calls.len();
            for (call, output_limit) in calls.into_iter().zip(limits) {
                if let Some(cause) = self.halted(control) {
                    return Err(Failure::stopped(cause, &prose));
                }
                self.stirred(strand, turn, turn::Motion::Running, active.clone());
                let result = self
                    .tooled(Address { strand, turn }, call, output_limit, control)
                    .await;
                if let Some(cause) = self.halted(control) {
                    return Err(Failure::stopped(cause, &prose));
                }
                attempt!(Operation::Tool, result);
            }
            timing.outputted(round, count);
        };

        Ok(Finish {
            last,
            response,
            settled: false,
            pause: None,
        })
    }
}
