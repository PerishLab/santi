use std::future::Future;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::watch::snippet;

pub(super) struct Spoken {
    pub(super) seq: i64,
    pub(super) stamp: String,
    pub(super) who: String,
    pub(super) lines: Vec<String>,
}

const KEPT: usize = 8000;

pub(super) fn lines(detail: Option<&serde_json::Value>, limit: usize) -> (usize, Vec<Spoken>) {
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

pub(super) fn message(placed: &serde_json::Value) -> Option<Spoken> {
    let role = placed.pointer("/message/role")?.as_str()?;
    let kind = placed.pointer("/message/kind")?.as_str()?;
    let text = placed.get("text")?.as_str()?;
    if text.trim().is_empty() {
        return None;
    }
    let who = match (role, kind) {
        ("soul", _) => "soul",
        ("system", "text") => "you",
        _ => "system",
    };
    let seq = placed
        .pointer("/relation/seq")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    let stamp = placed
        .pointer("/relation/created")
        .and_then(serde_json::Value::as_str)
        .map(clock)
        .unwrap_or_default();
    Some(Spoken {
        seq,
        stamp,
        who: who.to_string(),
        lines: readable(text, KEPT),
    })
}

pub(super) fn now() -> String {
    jiff::Zoned::now().strftime("%H:%M").to_string()
}

fn clock(raw: &str) -> String {
    let Ok(moment) = raw.parse::<jiff::Timestamp>() else {
        return String::new();
    };
    let here = moment.to_zoned(jiff::tz::TimeZone::system());
    let today = jiff::Zoned::now().date();
    if here.date() == today {
        here.strftime("%H:%M").to_string()
    } else {
        here.strftime("%m-%d %H:%M").to_string()
    }
}

fn readable(text: &str, limit: usize) -> Vec<String> {
    let kept = text
        .chars()
        .map(|held| match held {
            '\n' | '\t' => held,
            held if held.is_control() => ' ',
            held => held,
        })
        .collect::<String>();
    let mut out = kept.chars().take(limit).collect::<String>();
    if kept.chars().count() > limit {
        out.push('…');
    }
    out.trim_end()
        .split('\n')
        .map(str::to_string)
        .collect::<Vec<_>>()
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

pub(super) async fn read(
    request: impl Future<Output = Result<serde_json::Value>>,
    timeout: Duration,
) -> String {
    let Ok(Ok(value)) = tokio::time::timeout(timeout, request).await else {
        return "ctx --".to_string();
    };
    let Ok(total) = number(&value, "/estimate/total") else {
        return "ctx ?".to_string();
    };
    match value
        .pointer("/budget/bytes")
        .and_then(serde_json::Value::as_i64)
    {
        Some(cap) if cap > 0 => format!("ctx {}%", (total * 100 / cap).clamp(0, 999)),
        Some(_) => "ctx 0%".to_string(),
        None => format!("ctx {} B", total),
    }
}
