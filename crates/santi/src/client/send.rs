use anyhow::Result;
use std::io::Write;

use super::TIMEOUT;
use crate::cli::WatchFormat;
use crate::watch::Presentation;

pub struct Request<'a> {
    pub target: Target<'a>,
    pub body: serde_json::Value,
    pub watch: bool,
}

struct Accepted(String);
pub(crate) struct Completion(pub(crate) String);

pub(crate) enum Proof {
    Pending(String),
    Completed,
    Failed,
}

#[derive(Clone, Copy)]
pub struct Target<'a> {
    pub client: &'a reqwest::Client,
    pub base: &'a str,
    pub strand: &'a str,
    pub(crate) presentation: Presentation,
}

impl<'a> Target<'a> {
    pub fn new(
        client: &'a reqwest::Client,
        base: &'a str,
        strand: &'a str,
        format: WatchFormat,
    ) -> Self {
        Self {
            client,
            base,
            strand,
            presentation: Presentation::Watch(format),
        }
    }

    pub(crate) fn tui(client: &'a reqwest::Client, base: &'a str, strand: &'a str) -> Self {
        Self {
            client,
            base,
            strand,
            presentation: Presentation::Tui,
        }
    }
}

pub async fn send(request: Request<'_>) -> Result<()> {
    emit(request, &mut std::io::stdout()).await
}

pub async fn emit(request: Request<'_>, output: &mut impl Write) -> Result<()> {
    emission(request, output).await.map(|_| ())
}

pub(crate) async fn emission(
    request: Request<'_>,
    output: &mut impl Write,
) -> Result<Option<Completion>> {
    let watch = if request.watch {
        Some(crate::watch::subscribe(request.target).await?)
    } else {
        None
    };
    let url = format!(
        "{}/api/v1/strands/{}/send",
        request.target.base, request.target.strand
    );
    let response = request
        .target
        .client
        .post(&url)
        .timeout(TIMEOUT)
        .json(&request.body)
        .send()
        .await;
    let response = match response {
        Ok(response) => response,
        Err(error) if request.watch => {
            return Err(unknown(
                request.target,
                format!("POST did not return a response: {error}"),
            ));
        }
        Err(error) => return Err(anyhow::Error::new(error).context(format!("POST {url}"))),
    };
    let status = response.status();
    let text = response.text().await;
    if status.is_client_error() && request.watch {
        let detail = text
            .map(|text| crate::watch::snippet(&text, 500))
            .map_or_else(
                |error| format!("the response body was unreadable: {error}"),
                |body| format!("response body `{body}`"),
            );
        anyhow::bail!(
            "strand send outcome=not_accepted: POST was rejected with status {status}; {detail}"
        );
    }
    let text = text.map_err(|error| {
        if request.watch {
            unknown(
                request.target,
                format!("POST response body after status {status} was unreadable: {error}"),
            )
        } else {
            anyhow::Error::new(error).context("read response body")
        }
    })?;
    let accepted = serde_json::from_str::<serde_json::Value>(&text).ok();
    if !status.is_success() {
        if request.watch {
            let body = crate::watch::snippet(&text, 500);
            let detail = if body.is_empty() {
                format!("POST returned status {status} with an empty response body")
            } else {
                format!("POST returned status {status} with response body `{body}`")
            };
            return Err(unknown(request.target, detail));
        }
        print_response(output, accepted.as_ref(), &text)?;
        anyhow::bail!("request failed with status {status}");
    }
    if let Some(warning) = accepted_warning(accepted.as_ref()) {
        let proof = acceptance(accepted.as_ref(), request.target)?;
        print_response(output, accepted.as_ref(), &text)?;
        return Err(accepted_warning_error(warning, &proof.0));
    }
    if !request.watch {
        print_response(output, accepted.as_ref(), &text)?;
        return Ok(None);
    }
    let accepted = acceptance(accepted.as_ref(), request.target)?;
    let watch = watch.expect("watched send subscribed");
    let receipt = accepted.0;
    crate::watch::follow(watch, receipt.clone(), output).await?;
    Ok(Some(Completion(receipt)))
}

fn acceptance(value: Option<&serde_json::Value>, target: Target<'_>) -> Result<Accepted> {
    let Some(value) = value else {
        return Err(unknown(
            target,
            "successful response was not valid JSON".to_string(),
        ));
    };
    let receipt = exact(value, "/receipt/inbox").ok_or_else(|| {
        unknown(
            target,
            "successful response did not contain a valid exact receipt id".to_string(),
        )
    })?;
    let strand = exact(value, "/receipt/strand").ok_or_else(|| {
        unknown(
            target,
            "successful response did not contain a valid receipt strand".to_string(),
        )
    })?;
    if strand != target.strand {
        return Err(unknown(
            target,
            format!("successful response receipt belongs to strand {strand}"),
        ));
    }
    match value.get("turn") {
        Some(serde_json::Value::Null) => {}
        Some(_) => {
            exact(value, "/turn/id").ok_or_else(|| {
                unknown(
                    target,
                    "successful response did not contain a valid exact turn id".to_string(),
                )
            })?;
        }
        None => {
            return Err(unknown(
                target,
                "successful response did not classify its initial turn".to_string(),
            ));
        }
    }
    Ok(Accepted(receipt))
}

fn exact(value: &serde_json::Value, path: &str) -> Option<String> {
    value
        .pointer(path)
        .and_then(serde_json::Value::as_str)
        .filter(|held| !held.is_empty() && held.trim() == *held)
        .map(str::to_string)
}

fn unknown(target: Target<'_>, detail: String) -> anyhow::Error {
    anyhow::anyhow!(
        "strand send outcome=state_unknown: {detail}; the message may have been accepted, but its exact receipt was not proven; do not resend it; inspect with `santi strand runtime {}` and recover the accepted message's receipt/status before resuming or explicitly redriving with `santi strand drive {}`",
        target.strand,
        target.strand
    )
}

pub(crate) async fn prove(target: Target<'_>, receipt: &str) -> Result<Proof> {
    let url = format!("{}/api/v1/receipts/{receipt}", target.base);
    let response = target
        .client
        .get(&url)
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|error| uncertain(target, receipt, format!("GET failed: {error}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| uncertain(target, receipt, format!("response was unreadable: {error}")))?;
    if !status.is_success() {
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
        Some("failed") => Ok(Proof::Failed),
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

pub(crate) fn uncertain(target: Target<'_>, receipt: &str, detail: String) -> anyhow::Error {
    anyhow::anyhow!(
        "watch outcome=state_unknown: {detail} for accepted receipt {receipt}; do not resend the accepted message; inspect with `santi receipt {receipt}` and `santi strand runtime {}`; resume the blocking condition or explicitly redrive with `santi strand drive {}`",
        target.strand,
        target.strand
    )
}

fn field<'a>(value: &'a serde_json::Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(serde_json::Value::as_str)
}

pub(super) fn accepted_warning(value: Option<&serde_json::Value>) -> Option<&serde_json::Value> {
    value?
        .pointer("/receipt/warning")
        .filter(|warning| !warning.is_null())
}

pub(super) fn accepted_warning_error(warning: &serde_json::Value, receipt: &str) -> anyhow::Error {
    if let Some(command) = warning
        .pointer("/context/recovery/command")
        .and_then(serde_json::Value::as_str)
    {
        anyhow::anyhow!(
            "message was accepted as exact receipt {receipt} but not driven; do not resend it; run `{command}`"
        )
    } else {
        let code = warning
            .get("code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("unknown warning");
        anyhow::anyhow!(
            "message was accepted as exact receipt {receipt} but not driven; do not resend it; inspect and resolve `{code}`"
        )
    }
}

fn print_response(
    output: &mut impl Write,
    accepted: Option<&serde_json::Value>,
    text: &str,
) -> Result<()> {
    match accepted {
        Some(value) => writeln!(output, "{}", serde_json::to_string_pretty(value)?)?,
        None => writeln!(output, "{text}")?,
    }
    Ok(())
}
