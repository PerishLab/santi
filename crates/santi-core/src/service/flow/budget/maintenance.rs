use crate::service::{Service, Settlement};

impl Service {
    pub(in crate::service) async fn reserve(
        &self,
        strand: &str,
        turn: &str,
        round: usize,
        calls: usize,
    ) -> Result<bool, String> {
        let Some(budget) = self.rationed(strand) else {
            return Ok(false);
        };
        if budget.rounds < 2 || self.settlement(turn).is_some() {
            return Ok(false);
        }
        let usage = self.usage(strand).await?;
        let ceiling = budget.calls - (budget.calls / 8).max(1);
        let output = budget.output - (budget.output / 8).max(1);
        if usage.calls.saturating_add(calls) <= ceiling
            && output.saturating_sub(usage.output) >= calls
        {
            return Ok(false);
        }
        let reason = if usage.calls.saturating_add(calls) > ceiling {
            "calls"
        } else {
            "output"
        };
        let detail = format!(
            "<system_message>\nkind: execution_maintenance\nturn: {turn}\nprovider_round: {round}\nrequested_calls: {calls}\nreason: {reason}\nstate: This tool batch was not admitted into the reserved allowance. Only compact is available. A successful compact ends this turn and queues a pause notice for this strand.\n</system_message>"
        );
        self.settlements.lock().unwrap().insert(
            turn.to_string(),
            Settlement {
                reason: reason.into(),
                detail,
            },
        );
        Ok(true)
    }

    pub(in crate::service) fn settlement(&self, turn: &str) -> Option<String> {
        self.settlements
            .lock()
            .unwrap()
            .get(turn)
            .map(|state| state.detail.clone())
    }

    pub(in crate::service) fn reason(&self, turn: &str) -> Option<String> {
        self.settlements
            .lock()
            .unwrap()
            .get(turn)
            .map(|state| state.reason.clone())
    }

    pub(in crate::service) fn unsettle(&self, turn: &str) {
        self.settlements.lock().unwrap().remove(turn);
    }

    pub(in crate::service) async fn settling(
        &self,
        strand: &str,
        turn: &str,
        round: usize,
    ) -> Result<(), String> {
        let Some(budget) = self.rationed(strand) else {
            return Ok(());
        };
        if budget.rounds < 2 || self.settlement(turn).is_some() {
            return Ok(());
        }
        let usage = self.usage(strand).await?;
        let reason = if round > budget.rounds - (budget.rounds / 8).max(1) {
            Some("provider_rounds")
        } else if usage.calls >= budget.calls - (budget.calls / 8).max(1) {
            Some("calls")
        } else if usage.output >= budget.output - (budget.output / 8).max(1) {
            Some("output")
        } else {
            None
        };
        if let Some(reason) = reason {
            let detail = format!(
                "<system_message>\nkind: execution_maintenance\nturn: {turn}\nreason: {reason}\nprovider_round: {round}\nprovider_round_limit: {}\ncalls: {}\noutput_bytes: {}\nstate: Only compact is available. A successful compact ends this turn and queues a pause notice for this strand.\n</system_message>",
                budget.rounds, usage.calls, usage.output
            );
            self.settlements.lock().unwrap().insert(
                turn.to_string(),
                Settlement {
                    reason: reason.into(),
                    detail,
                },
            );
        }
        Ok(())
    }
}
