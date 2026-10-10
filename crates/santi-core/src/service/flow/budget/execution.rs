use super::*;

pub(in crate::service) enum Verdict {
    Unbounded,
    Maintain,
    Bounded(Vec<usize>),
    Rejected(Box<Fault>),
}

impl Service {
    pub(in crate::service) async fn incomplete(
        &self,
        strand: &str,
        turn: &str,
        round: usize,
    ) -> Result<Fault, String> {
        let budget = self
            .rationed(strand)
            .ok_or_else(|| "maintenance budget missing".to_string())?;
        let usage = self.usage(strand).await?;
        self.breached(Breach {
            strand,
            turn,
            budget: &budget,
            usage,
            reason: "compact_required",
            request: json!({ "provider_round": round, "completion": "without_compact" }),
        })
        .await
    }

    pub(in crate::service) async fn readmit(
        &self,
        strand: &str,
        turn: &str,
        next: usize,
    ) -> Result<Option<Fault>, String> {
        let Some(budget) = self.rationed(strand) else {
            return Ok(None);
        };
        if next <= budget.rounds {
            return Ok(None);
        }
        let usage = self.usage(strand).await?;
        self.breached(Breach {
            strand,
            turn,
            budget: &budget,
            usage,
            reason: "provider_rounds",
            request: json!({"next_provider_round": next}),
        })
        .await
        .map(Some)
    }

    pub(in crate::service) async fn judge(
        &self,
        strand: &str,
        turn: &str,
        round: usize,
        calls: usize,
    ) -> Result<Verdict, String> {
        let Some(budget) = self.rationed(strand) else {
            return Ok(Verdict::Unbounded);
        };
        let usage = self.usage(strand).await?;
        let request = json!({
            "provider_round": round,
            "calls": calls,
        });
        if round >= budget.rounds && self.settlement(turn).is_none() {
            return self
                .breached(Breach {
                    strand,
                    turn,
                    budget: &budget,
                    usage,
                    reason: "provider_rounds",
                    request,
                })
                .await
                .map(Box::new)
                .map(Verdict::Rejected);
        }
        if self.reserve(strand, turn, round, calls).await? {
            return Ok(Verdict::Maintain);
        }
        if usage.calls.saturating_add(calls) > budget.calls {
            return self
                .breached(Breach {
                    strand,
                    turn,
                    budget: &budget,
                    usage,
                    reason: "calls",
                    request,
                })
                .await
                .map(Box::new)
                .map(Verdict::Rejected);
        }
        let ceiling = if budget.rounds >= 2 && self.settlement(turn).is_none() {
            budget.output - (budget.output / 8).max(1)
        } else {
            budget.output
        };
        let room = ceiling.saturating_sub(usage.output);
        if room < calls {
            return self
                .breached(Breach {
                    strand,
                    turn,
                    budget: &budget,
                    usage,
                    reason: "output",
                    request,
                })
                .await
                .map(Box::new)
                .map(Verdict::Rejected);
        }
        Ok(Verdict::Bounded(allotted(room, budget.shell, calls)))
    }

    pub(in crate::service) async fn usage(&self, strand: &str) -> Result<budget::Usage, String> {
        let tally = self.store.tally(strand).await?;
        let output = tally.spent.into_iter().fold(0usize, |held, spent| {
            held.saturating_add(match spent {
                santi_estate::Spent::Output(output) => captured(&output),
                santi_estate::Spent::Error(error) => error.len(),
            })
        });
        Ok(budget::Usage {
            calls: tally.calls,
            output,
        })
    }

    async fn breached(&self, breach: Breach<'_>) -> Result<Fault, String> {
        let Breach {
            strand,
            turn,
            budget,
            usage,
            reason,
            request,
        } = breach;
        let error = self
            .store
            .raise(
                santi_error::Draft {
                    key: crate::budget::Error::Execution
                        .descriptor()
                        .key("strand", strand),
                    descriptor: crate::budget::Error::Execution.descriptor(),
                    scope: santi_error::Scope::new("strand", strand),
                    source: santi_error::Source::new("santi-core", "turn.execution_budget"),
                    message: format!("strand execution budget exceeded: {reason}"),
                    context: json!({
                        "schema": "santi.error.execution_budget.v1",
                        "profile": budget.profile,
                        "reason": reason,
                        "turn": turn,
                        "limits": {
                            "provider_rounds": budget.rounds,
                            "calls": budget.calls,
                            "output": budget.output,
                            "shell_output_bytes": budget.shell,
                        },
                        "usage": {
                            "calls": usage.calls,
                            "output": usage.output,
                        },
                        "request": request,
                    }),
                },
                &crate::now(),
            )
            .await?;
        self.dispatched().await;
        Ok(error)
    }
}

struct Breach<'a> {
    strand: &'a str,
    turn: &'a str,
    budget: &'a budget::Execution,
    usage: budget::Usage,
    reason: &'a str,
    request: serde_json::Value,
}

fn allotted(total: usize, per_call: usize, calls: usize) -> Vec<usize> {
    let mut remaining = total;
    let mut limits = Vec::with_capacity(calls);
    for index in 0..calls {
        let left = calls - index;
        let limit = (remaining / left).min(per_call);
        limits.push(limit);
        remaining -= limit;
    }
    limits
}

fn captured(output: &serde_json::Value) -> usize {
    let stdout = output
        .get("stdout")
        .and_then(serde_json::Value::as_str)
        .map_or(0, str::len);
    let stderr = output
        .get("stderr")
        .and_then(serde_json::Value::as_str)
        .map_or(0, str::len);
    if output.get("stdout").is_some() || output.get("stderr").is_some() {
        stdout.saturating_add(stderr)
    } else {
        output.to_string().len()
    }
}
