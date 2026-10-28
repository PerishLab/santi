use serde_json::json;

pub(in crate::service) mod slots;

use super::Service;
use crate::{budget, compact};

impl Service {
    pub async fn exec(
        &self,
        strand: &str,
        request: compact::Exec,
    ) -> Result<compact::Report, String> {
        let summary = request.summary.trim();
        if summary.is_empty() {
            return Err("compact summary must not be empty".to_string());
        }
        let strand = self
            .store
            .strand(strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let policy = self.regimen();
        let choice = self
            .store
            .selection(
                &strand.id,
                &request,
                santi_estate::Limits {
                    slots: policy.slots,
                    settled: policy.settled,
                },
            )
            .await?;
        let (from, to) = (&choice.range.first, &choice.range.last);
        let before = self.estimate(&strand.id).await?;
        let compact_tag = crate::tag("compact");
        let mut preview = self
            .store
            .preview_compact(&compact_tag, &strand.id, from, to)
            .await?;
        preview.before = Some(before.clone());
        preview.requested = choice.requested.clone();
        let (metadata, after, ratio) = self
            .forecast(&strand.id, &preview, summary, request.capsule.as_ref())
            .await?;
        if request.dry {
            preview.after = Some(after);
            preview.ratio = ratio;
            return Ok(preview);
        }
        let mut response = self
            .store
            .create_compact(santi_estate::CompactDraft {
                tag: &compact_tag,
                strand: &strand.id,
                first: from,
                last: to,
                summary,
                metadata: Some(&metadata),
                expected: Some(&preview),
                created: &crate::now(),
            })
            .await?;
        self.rearm(&strand.id);
        match self.estimate(&strand.id).await {
            Ok(after) => {
                response.ratio = squeezed(&before, &after);
                response.after = Some(after);
            }
            Err(error) => eprintln!("santi: committed compact estimate failed: {error}"),
        }
        match self.absolve(&strand.id, "compact_exec").await {
            Ok(resolved) => response.active_incident_resolved = resolved,
            Err(error) => eprintln!("santi: committed compact resolution failed: {error}"),
        }
        if response.active_incident_resolved {
            self.poked(&strand.id, "strand_send", None, "compact_recovery_poke")
                .await;
        }
        response.before = Some(before);
        response.requested = choice.requested;
        Ok(response)
    }

    async fn forecast(
        &self,
        strand: &str,
        response: &compact::Report,
        summary: &str,
        capsule: Option<&compact::Capsule>,
    ) -> Result<(serde_json::Value, budget::Estimate, Option<f64>), String> {
        let before = response
            .before
            .as_ref()
            .ok_or_else(|| "compact estimate missing".to_string())?;
        let proposed = metadata(Forecast {
            capsule,
            response: Some(response),
            before: Some(before),
            after: None,
            budget: self.budget().as_ref(),
            ratio: None,
        });
        let after = self.foreseen(strand, response, summary, proposed).await?;
        let ratio = squeezed(before, &after);
        let proposed = metadata(Forecast {
            capsule,
            response: Some(response),
            before: Some(before),
            after: Some(&after),
            budget: self.budget().as_ref(),
            ratio,
        });
        let after = self.foreseen(strand, response, summary, proposed).await?;
        let ratio = squeezed(before, &after);
        let metadata = metadata(Forecast {
            capsule,
            response: Some(response),
            before: Some(before),
            after: Some(&after),
            budget: self.budget().as_ref(),
            ratio,
        });
        Ok((metadata, after, ratio))
    }

    async fn foreseen(
        &self,
        strand: &str,
        response: &compact::Report,
        summary: &str,
        metadata: serde_json::Value,
    ) -> Result<budget::Estimate, String> {
        let input = crate::provider_preview(
            &self.store,
            strand,
            crate::ProviderPreview {
                report: response,
                summary,
                metadata: &metadata,
            },
        )
        .await?;
        let instructions = self.wording(strand).await?;
        let tools = self.offered(strand).await?;
        let mut request = self.prepare(input, instructions, tools)?;
        self.provider.validate(&request)?;
        self.suffix(&mut request);
        Ok(crate::context::budget::gauged(
            &request,
            self.provider.as_ref(),
        ))
    }

    pub async fn page(
        &self,
        compact: &str,
        keyword: Option<&str>,
        page_index: i64,
        page_size: i64,
    ) -> Result<Option<compact::Page>, String> {
        self.store
            .compact_page(compact, keyword, page_index, page_size)
            .await
    }
}

struct Forecast<'a> {
    capsule: Option<&'a compact::Capsule>,
    response: Option<&'a compact::Report>,
    before: Option<&'a budget::Estimate>,
    after: Option<&'a budget::Estimate>,
    budget: Option<&'a budget::Cap>,
    ratio: Option<f64>,
}

const SOURCING: usize = 128;
const REASONING: usize = 512;
const RISK: usize = 1024;
const QUERYABILITY: usize = 512;

fn metadata(input: Forecast<'_>) -> serde_json::Value {
    let originals = input
        .response
        .map(|response| format!("santi compact query --compact {}", response.compact));
    let range = input.response.map(|response| {
        json!({
            "start_seq": response.from,
            "end_seq": response.to,
            "first": response.first,
            "last": response.last,
            "collapsed_count": response.collapsed,
            "absorbed": response.absorbed,
        })
    });
    let mut metadata = json!({
        "schema": if input.capsule.is_some() { "santi.compact_capsule.v1" } else { "santi.compact_estimate.v1" },
        "operation": if input.capsule.is_some() { "manual_capsule" } else { "manual_compact" },
        "compact": input.response.map(|response| &response.compact),
        "originals_query": originals,
        "range": range,
        "before": input.before,
        "forecast": input.after.map(|after| json!({
            "authoritative": false,
            "basis": "precommit_preview",
            "after": after,
            "ratio": input.ratio,
        })),
        "budget": input.budget,
    });
    if let Some(capsule) = input.capsule {
        metadata["declared_source"] = json!(capped(&capsule.source, SOURCING));
        metadata["source_trust"] = json!("caller_declared");
        metadata["reason"] = json!(capped(&capsule.reason, REASONING));
        metadata["risk"] = json!(capped(&capsule.risk, RISK));
        metadata["queryability"] = json!(
            capsule
                .queryability
                .as_ref()
                .map(|value| capped(value, QUERYABILITY))
        );
    }
    metadata
}

fn squeezed(before: &budget::Estimate, after: &budget::Estimate) -> Option<f64> {
    if before.total <= 0 {
        return None;
    }
    Some(after.total as f64 / before.total as f64)
}

fn capped(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let suffix = " [truncated]";
    let suffix = suffix.len();
    let mut end = max_bytes.saturating_sub(suffix).min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{}", &value[..end], suffix)
}
