use super::{Proof, TIMEOUT, Target, uncertain};
use anyhow::Result;

pub(crate) async fn prove(target: Target<'_>, receipt: &str) -> Result<Proof> {
    let url = format!("{}/api/v1/receipts/{receipt}", target.base);
    let response = target
        .client
        .get(&url)
        .timeout(TIMEOUT)
        .send()
        .await
        .inspect_err(crate::client::transport)
        .map_err(|error| {
            uncertain(
                target,
                receipt,
                format!("GET failed: {}", error.without_url()),
            )
        })?;
    let status = response.status();
    let body = response
        .text()
        .await
        .inspect_err(crate::client::transport)
        .map_err(|error| {
            uncertain(
                target,
                receipt,
                format!("response was unreadable: {}", error.without_url()),
            )
        })?;
    if !status.is_success() {
        crate::client::body(&body);
        return Err(uncertain(
            target,
            receipt,
            format!("GET returned status {status}"),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| uncertain(target, receipt, format!("invalid JSON: {error}")))?;
    if field(&value, "inbox") != Some(receipt) || field(&value, "strand") != Some(target.strand) {
        return Err(uncertain(
            target,
            receipt,
            "response identity did not match the accepted message".to_string(),
        ));
    }
    match field(&value, "state") {
        Some("completed") => Ok(Proof::Completed),
        Some("failed") => Ok(Proof::Failed(
            value
                .get("transitions")
                .and_then(serde_json::Value::as_array)
                .and_then(|transitions| {
                    transitions
                        .iter()
                        .rev()
                        .find(|held| field(held, "state") == Some("failed"))
                })
                .and_then(|held| field(held, "turn"))
                .filter(|turn| !turn.is_empty())
                .map(str::to_string),
        )),
        Some(state @ ("accepted" | "recovered" | "driving")) => {
            Ok(Proof::Pending(state.to_string()))
        }
        _ => Err(uncertain(
            target,
            receipt,
            "response did not contain a recognized durable state".to_string(),
        )),
    }
}

fn field<'a>(value: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(serde_json::Value::as_str)
}
