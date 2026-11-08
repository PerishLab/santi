use anyhow::Result;
use std::io::Write;

use super::TIMEOUT;
use crate::cli::WatchFormat;
use crate::watch::{Bytes, Emit, Presentation};

pub struct Request<'a> {
    pub target: Target<'a>,
    pub body: serde_json::Value,
    pub watch: bool,
}

mod fault;
pub(crate) mod guidance;
mod outcome;
mod proof;
pub(crate) use fault::{accepted_warning, accepted_warning_error, uncertain, unknown};
pub(crate) use outcome::{Failed, Unsettled, pending};
pub(crate) use proof::prove;

struct Accepted(String);
pub(crate) struct Completion(pub(crate) String);

pub(crate) enum Proof {
    Pending(String),
    Completed,
    Failed(Option<String>),
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
    emission(request, &mut Bytes(output)).await.map(|_| ())
}

pub(crate) async fn emission(
    request: Request<'_>,
    output: &mut impl Emit,
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
                format!("POST did not return a response: {}", error.without_url()),
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
                |error| format!("the response body was unreadable: {}", error.without_url()),
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
                format!(
                    "POST response body after status {status} was unreadable: {}",
                    error.without_url()
                ),
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
        return Err(accepted_warning_error(request.target, warning, &proof.0));
    }
    if !request.watch {
        print_response(output, accepted.as_ref(), &text)?;
        return Ok(None);
    }
    let accepted = acceptance(accepted.as_ref(), request.target)?;
    let watch = watch.expect("watched send subscribed");
    let receipt = accepted.0;
    crate::watch::follow(watch, receipt.clone(), output).await?;
    if matches!(
        request.target.presentation,
        Presentation::Watch(WatchFormat::Filtered)
    ) {
        writeln!(
            output,
            "receipt {receipt} completed for strand {}; this does not establish Strand idleness or business completion; {}",
            request.target.strand,
            guidance::inspect(request.target.base, request.target.strand, Some(&receipt))
        )?;
    }
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

fn print_response(
    output: &mut impl Emit,
    accepted: Option<&serde_json::Value>,
    text: &str,
) -> Result<()> {
    match accepted {
        Some(value) => writeln!(output, "{}", serde_json::to_string_pretty(value)?)?,
        None => writeln!(output, "{text}")?,
    }
    Ok(())
}
