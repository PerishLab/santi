use super::super::{ClientDefaults, Http, TIMEOUT};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::time::Duration;

pub(in crate::client) struct Reply<'a> {
    pub http: &'a Http<'a>,
    pub base: &'a str,
    pub defaults: &'a ClientDefaults,
}

impl Reply<'_> {
    pub(in crate::client) async fn run(&self, args: crate::cli::reply::Reply) -> Result<()> {
        let inbox = super::super::http::urlencoding_encode(&args.inbox);
        let url = format!("{}/api/v1/receipts/{inbox}/reply", self.base);
        let Some(text) = args.text else {
            return self.http.get(&url).await;
        };
        let operation = args.operation.context("reply requires operation")?;
        let strand = self.defaults.resolve_strand(None)?;
        let body = json!({"strand":strand,"operation":operation,"text":text});
        let response = self.http.client.post(&url).json(&body).timeout(TIMEOUT).send().await
            .context("reply outcome unknown; inspect exact receipt/operation; never create a new operation to retry")?;
        if !response.status().is_success() {
            return super::super::http::print_json(response).await;
        }
        let mut status: Value = response
            .json()
            .await
            .context("reply response incomplete; inspect exact receipt/operation")?;
        if !args.watch {
            return print(status);
        }
        let deadline = tokio::time::Instant::now() + TIMEOUT;
        loop {
            if self.finished(&status, &operation)? {
                return print(status);
            }
            if tokio::time::Instant::now() >= deadline {
                print(status)?;
                anyhow::bail!(
                    "reply pending or blocked; inspect `santi reply --inbox {}`; do not create another operation",
                    args.inbox
                );
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            let response = self
                .http
                .client
                .get(&url)
                .timeout(deadline.saturating_duration_since(tokio::time::Instant::now()))
                .send()
                .await?;
            if !response.status().is_success() {
                return super::super::http::print_json(response).await;
            }
            status = response.json().await?;
        }
    }

    fn finished(&self, status: &Value, operation: &str) -> Result<bool> {
        let attempts = status["window"]["attempts"]
            .as_array()
            .context("reply attempts missing")?;
        let attempt = attempts
            .iter()
            .find(|a| a["operation"] == operation)
            .context("reply operation missing")?;
        match attempt["state"].as_str() {
            Some("accepted" | "observed") => Ok(true),
            Some("rejected" | "unsent" | "unknown" | "not_applied" | "cancelled") => {
                print(status.clone())?;
                anyhow::bail!(
                    "reply operation did not establish acceptance; inspect retained result; never replay unknown writes"
                );
            }
            Some("queued" | "dispatching") => Ok(false),
            _ => anyhow::bail!("reply operation state unknown; inspect receipt"),
        }
    }
}

fn print(status: Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(())
}
