use super::{asked, clock};
use crate::service::{Service, address::Address};
use santi_estate::{Attempt, Invocation};
use santi_provider::Call;

impl Service {
    pub(in crate::service) async fn maintaining(
        &self,
        strand: &str,
        turn: &str,
    ) -> Result<bool, String> {
        Ok(self.settlement(turn).is_some() && !clock::selected(self, strand).await?)
    }

    pub(in crate::service) fn staged(
        &self,
        address: Address<&str>,
        calls: Vec<Call>,
        limits: Vec<Option<usize>>,
    ) -> Result<Vec<Invocation>, String> {
        let detail = self
            .settlement(address.turn)
            .ok_or_else(|| "maintenance owner missing".to_string())?;
        let mut invocations = Vec::with_capacity(calls.len());
        for (call, limit) in calls.into_iter().zip(limits) {
            let attempt = match call.name.as_str() {
                "compact" => match asked(&call) {
                    Ok(request) => Attempt::Compact(Box::new(request)),
                    Err(error) => Attempt::Rejected(error),
                },
                "shell" => Attempt::Rejected(detail.clone()),
                _ => Attempt::Rejected(format!(
                    "unsupported tool on current runtime state: {}",
                    call.name
                )),
            };
            invocations.push(Invocation {
                call: crate::tool::Call {
                    id: call.call,
                    turn: address.turn.into(),
                    tool: call.name,
                    arguments: call.arguments,
                    created: crate::now(),
                },
                result: crate::tag("result"),
                attempt,
                limit,
            });
        }
        Ok(invocations)
    }
}
