use crate::Ruled;
use serde_json::json;

use crate::Fault;
use crate::context::budget::estimated;
use santi_provider::Item;

use super::super::Service;
use crate::budget;

mod execution;
mod maintenance;
mod observation;
#[cfg(test)]
mod ownership;
pub(super) use execution::Verdict;

const PROVIDER: &str = "provider_request_exceeds_budget";

struct Pressure<'a> {
    strand: &'a str,
    code: &'a str,
    text: &'a str,
    operation: &'a str,
    provider: Option<&'a str>,
    model: Option<&'a str>,
    source: Option<&'a str>,
    bytes: Option<i64>,
    estimate: &'a budget::Estimate,
    observed: Option<&'a str>,
    metadata: serde_json::Value,
}

impl Service {
    pub(in crate::service) fn budget(&self) -> Option<budget::Cap> {
        self.provider.metadata().budget.map(|budget| budget::Cap {
            bytes: budget.bytes as i64,
            source: budget.source,
        })
    }

    pub(in crate::service) async fn loaded(&self, strand: &str) -> Result<i64, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
        input.extend(self.pending_input(strand).await?);
        Ok(estimated(&input, None, None).input)
    }

    pub(in crate::service) async fn estimate(
        &self,
        strand: &str,
    ) -> Result<budget::Estimate, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
        input.extend(self.pending_input(strand).await?);
        let instructions = self.wording(strand).await?;
        let tools = self.offered(strand).await?;
        Ok(estimated(&input, Some(&instructions), Some(&tools)))
    }

    async fn pending_input(&self, strand: &str) -> Result<Vec<Item>, String> {
        Ok(self
            .store
            .inboxes(strand)
            .await?
            .into_iter()
            .filter_map(|inbox| {
                let content = inbox.content.rendered();
                (!content.trim().is_empty()).then(|| Item::Message {
                    role: match inbox.kind {
                        crate::message::Kind::Text => "user",
                        crate::message::Kind::SantiSystem => "system",
                    }
                    .to_string(),
                    content,
                })
            })
            .collect())
    }

    pub(in crate::service) async fn admit_candidate(
        &self,
        strand: &str,
        kind: &crate::message::Kind,
        content: &crate::message::Content,
    ) -> Result<Option<Fault>, String> {
        let Some(budget) = self.budget() else {
            return Ok(None);
        };
        let mut input = crate::provider_input(&self.store, strand).await?;
        input.extend(self.pending_input(strand).await?);
        if let Some(candidate) = crate::context::budget::inbound(kind, content) {
            input.push(candidate);
        }
        let tools = self.offered(strand).await?;
        let estimate = estimated(&input, Some(&self.wording(strand).await?), Some(&tools));
        if estimate.total <= budget.bytes {
            return Ok(None);
        }
        let metadata = self.provider.metadata();
        let reason = reason(estimate.total, budget.bytes);
        self.pressure(Pressure {
            strand,
            code: "candidate_input_exceeds_budget",
            text: &reason,
            operation: "ingest_admission",
            provider: Some(metadata.provider.as_ref()),
            model: Some(&metadata.model),
            source: Some(&budget.source),
            bytes: Some(budget.bytes),
            estimate: &estimate,
            observed: None,
            metadata: serde_json::json!({"estimator": estimate.estimator}),
        })
        .await
        .map(Some)
    }

    pub(in crate::service) async fn admit_pending(
        &self,
        strand: &crate::strand::Strand,
        operation: &str,
    ) -> Result<Option<Fault>, String> {
        let Some(budget) = self.budget() else {
            return Ok(None);
        };
        let key = crate::budget::Error::Context
            .descriptor()
            .key("strand", &strand.id);
        let active = self.store.incident(&key).await?;
        let mut observed = None;
        if let Some(incident) = active.as_ref() {
            let current = observation::observe(self, strand, &budget).await?;
            if operation == "cold_start_resume" && current.matches(incident) {
                return Ok(Some(crate::engine().fault(incident)));
            }
            observed = Some(current);
        }
        let estimate = self.estimate(&strand.id).await?;
        if estimate.total <= budget.bytes {
            if active.is_some() {
                resolve(self, &key, "driver_remeasurement", &estimate).await?;
            }
            return Ok(None);
        }
        let observed = match observed {
            Some(observed) => observed,
            None => observation::observe(self, strand, &budget).await?,
        };
        let metadata = self.provider.metadata();
        let reason = reason(estimate.total, budget.bytes);
        self.pressure(Pressure {
            strand: &strand.id,
            code: "pending_drain_would_exceed_budget",
            text: &reason,
            operation: "pending_drain_admission",
            provider: Some(metadata.provider.as_ref()),
            model: Some(&metadata.model),
            source: Some(&budget.source),
            bytes: Some(budget.bytes),
            estimate: &estimate,
            observed: None,
            metadata: serde_json::json!({
                "estimator": estimate.estimator,
                "observation": observed,
            }),
        })
        .await
        .map(Some)
    }

    pub(in crate::service) async fn overdrawn(
        &self,
        strand: &str,
        turn: &str,
        request: &santi_provider::Request,
        estimate: &budget::Estimate,
    ) -> Result<Option<Fault>, String> {
        let Some(budget) = self.budget() else {
            return Ok(None);
        };
        if estimate.total <= budget.bytes {
            return Ok(None);
        }
        let metadata = self.provider.metadata();
        let strand = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let reason = reason(estimate.total, budget.bytes);
        let error = self
            .pressure(Pressure {
                strand: &strand.id,
                code: PROVIDER,
                text: &reason,
                operation: "provider_preflight",
                provider: Some(metadata.provider.as_ref()),
                model: Some(&request.model),
                source: Some(&budget.source),
                bytes: Some(budget.bytes),
                estimate,
                observed: Some(turn),
                metadata: json!({
                    "phase": "provider_preflight",
                    "estimator": estimate.estimator,
                    "observed_at_seq": strand.next - 1,
                }),
            })
            .await?;
        self.dispatched().await;
        Ok(Some(error))
    }

    async fn pressure(&self, pressure: Pressure<'_>) -> Result<Fault, String> {
        self.store
            .raise(
                santi_error::Draft {
                    key: crate::budget::Error::Context
                        .descriptor()
                        .key("strand", pressure.strand),
                    descriptor: crate::budget::Error::Context.descriptor(),
                    scope: santi_error::Scope::new("strand", pressure.strand),
                    source: santi_error::Source::new("santi-core", pressure.operation),
                    message: pressure.text.to_string(),
                    context: json!({
                        "schema": "santi.error.context_budget.v1",
                        "reason": pressure.code,
                        "provider": pressure.provider,
                        "model": pressure.model,
                        "budget": {"source": pressure.source, "input": pressure.bytes},
                        "estimate": pressure.estimate,
                        "observed_turn_id": pressure.observed,
                        "details": pressure.metadata,
                    }),
                },
                &crate::now(),
            )
            .await
    }

    pub(in crate::service) async fn absolve(
        &self,
        strand: &str,
        cleared_by: &str,
    ) -> Result<bool, String> {
        let key = crate::budget::Error::Context
            .descriptor()
            .key("strand", strand);
        if self.store.incident(&key).await?.is_none() {
            return Ok(false);
        }
        let Some(budget) = self.budget() else {
            return Ok(false);
        };
        let estimate = self.estimate(strand).await?;
        if estimate.total > budget.bytes {
            return Ok(false);
        }
        resolve(self, &key, cleared_by, &estimate).await
    }
}

async fn resolve(
    service: &Service,
    key: &str,
    cleared_by: &str,
    estimate: &budget::Estimate,
) -> Result<bool, String> {
    let resolved = service
        .store
        .resolve(
            key,
            cleared_by,
            json!({
                "schema": "santi.error.context_budget.resolution.v1",
                "resolved_by": cleared_by,
                "estimate": estimate,
            }),
            &crate::now(),
        )
        .await?;
    service.dispatched().await;
    Ok(resolved)
}

fn reason(total: i64, bytes: i64) -> String {
    format!("strand context is over budget ({total} estimated bytes, budget {bytes})")
}
