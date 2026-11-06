use super::maintenance::Maintenance;
use crate::service::flow::budget::Verdict;
use crate::service::flow::failure::{Admission, Failure, Operation};
use crate::service::{Service, address::Address, interrupt::Control, timing};
use crate::{message, turn};
use santi_provider::Call;

pub(super) struct Invocation<'a, 'turn> {
    pub(super) address: Address<&'a str>,
    pub(super) round: usize,
    pub(super) calls: Vec<Call>,
    pub(super) reply: Option<&'a message::Placed>,
    pub(super) response: Option<&'a str>,
    pub(super) active: Option<String>,
    pub(super) prose: &'a str,
    pub(super) timing: &'a timing::Turn<'turn>,
}

impl Service {
    pub(super) async fn invoke(
        &self,
        invocation: Invocation<'_, '_>,
        control: &Control,
    ) -> Result<bool, Failure> {
        let Invocation {
            address,
            round,
            calls,
            reply,
            response,
            active,
            prose,
            timing,
        } = invocation;
        let Address { strand, turn } = address;
        let verdict = self
            .judge(strand, turn, round, calls.len())
            .await
            .map_err(|error| {
                Failure::runtime(Operation::Admission(Admission::Execution), error, prose)
            })?;
        let limits = match verdict {
            Verdict::Maintain => return Ok(false),
            Verdict::Unbounded => vec![None; calls.len()],
            Verdict::Bounded(limits) => limits.into_iter().map(Some).collect::<Vec<_>>(),
            Verdict::Rejected(error) => return Err(Failure::execution(*error, prose)),
        };
        let maintenance = self
            .maintaining(turn)
            .await
            .map_err(|error| Failure::runtime(Operation::Assembly, error, prose))?;
        let count = calls.len();
        timing.outputting(round, count);
        if maintenance {
            self.stirred(strand, turn, turn::Motion::Running, active.clone());
            self.activity.update(
                strand,
                turn::activity::Current {
                    turn: turn.to_string(),
                    round,
                    phase: turn::activity::Phase::Invoking,
                    call: None,
                },
            );
            let settled = self
                .maintained(
                    Maintenance {
                        address,
                        round,
                        calls,
                        limits,
                        reply,
                        response,
                    },
                    control,
                )
                .await?;
            timing.outputted(round, count);
            return Ok(settled);
        }
        for (call, output_limit) in calls.into_iter().zip(limits) {
            if let Some(cause) = self.halted(control) {
                return Err(Failure::stopped(cause, prose));
            }
            self.stirred(strand, turn, turn::Motion::Running, active.clone());
            self.activity.update(
                strand,
                turn::activity::Current {
                    turn: turn.to_string(),
                    round,
                    phase: turn::activity::Phase::Invoking,
                    call: Some(call.call.clone()),
                },
            );
            let result = self
                .tooled(Address { strand, turn }, call, output_limit, control)
                .await;
            if let Some(cause) = self.halted(control) {
                return Err(Failure::stopped(cause, prose));
            }
            result.map_err(|error| Failure::runtime(Operation::Tool, error, prose))?;
        }
        timing.outputted(round, count);
        Ok(false)
    }
}
