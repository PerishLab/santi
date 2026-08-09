use std::future::Future;
use std::time::Duration;

use anyhow::Result;

use super::parse::number;

pub(super) async fn read(
    request: impl Future<Output = Result<serde_json::Value>>,
    timeout: Duration,
) -> String {
    let Ok(Ok(value)) = tokio::time::timeout(timeout, request).await else {
        return "context: unavailable".to_string();
    };
    let Ok(total) = number(&value, "/estimate/total") else {
        return "context: unread".to_string();
    };
    match value
        .pointer("/budget/bytes")
        .and_then(serde_json::Value::as_i64)
    {
        Some(cap) => format!("context: {total}/{cap} bytes"),
        None => format!("context: {total} bytes (unbounded)"),
    }
}
