use anyhow::{Context, Result};

use crate::watch::snippet;

pub(super) fn lines(detail: Option<&serde_json::Value>, limit: usize) -> (usize, Vec<String>) {
    let Some(messages) = detail
        .and_then(|detail| detail.get("messages"))
        .and_then(serde_json::Value::as_array)
    else {
        return (0, Vec::new());
    };
    let omitted = messages.len().saturating_sub(limit);
    let spoken = messages
        .iter()
        .skip(omitted)
        .filter_map(message)
        .collect::<Vec<_>>();
    (omitted, spoken)
}

pub(super) fn message(placed: &serde_json::Value) -> Option<String> {
    let role = placed.pointer("/message/role")?.as_str()?;
    let kind = placed.pointer("/message/kind")?.as_str()?;
    let text = placed.get("text")?.as_str()?;
    if text.trim().is_empty() {
        return None;
    }
    let speaker = match (role, kind) {
        ("soul", _) => "soul",
        ("system", "text") => "you",
        _ => "system",
    };
    Some(format!("{speaker}> {}", snippet(text, 2000)))
}

pub(super) async fn decode(response: reqwest::Response) -> Result<serde_json::Value> {
    let status = response.status();
    let body = response.text().await.context("read response body")?;
    if !status.is_success() {
        anyhow::bail!(
            "request failed with status {status}: {}",
            snippet(&body, 500)
        );
    }
    serde_json::from_str(&body).context("decode response json")
}

pub(super) fn text<'a>(value: &'a serde_json::Value, path: &str) -> Result<&'a str> {
    value
        .pointer(path)
        .and_then(serde_json::Value::as_str)
        .filter(|held| !held.is_empty() && held.trim() == *held)
        .ok_or_else(|| anyhow::anyhow!("response missing valid {path}"))
}

pub(super) fn number(value: &serde_json::Value, path: &str) -> Result<i64> {
    value
        .pointer(path)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("response missing {path}"))
}
