use std::time::Instant;

use reqwest::Response;
use serde_json::Value;

use crate::{Receipt, Usage};

pub const RECEIPT: &[&str] = &["x-request-id"];

pub const DENIED: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

pub(crate) fn received(response: &Response, allowed: &[String], sent: Instant) -> Receipt {
    let headers = allowed
        .iter()
        .map(|name| name.to_ascii_lowercase())
        .filter(|name| !DENIED.contains(&name.as_str()))
        .filter_map(|name| {
            let value = response.headers().get(name.as_str())?.to_str().ok()?;
            Some((name, value.to_string()))
        })
        .collect();
    Receipt {
        status: response.status().as_u16(),
        elapsed: u64::try_from(sent.elapsed().as_millis()).unwrap_or(u64::MAX),
        headers,
    }
}

pub(crate) fn counted(usage: &Value, input: &str, output: &str) -> Usage {
    let count = |path: &[&str]| {
        path.iter()
            .try_fold(usage, |value, key| value.get(key))
            .and_then(Value::as_u64)
    };
    Usage {
        input: count(&[input]),
        cached: count(&[&format!("{input}_details"), "cached_tokens"]),
        output: count(&[output]),
        reasoning: count(&[&format!("{output}_details"), "reasoning_tokens"]),
    }
}
