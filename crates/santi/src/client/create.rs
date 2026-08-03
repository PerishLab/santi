use anyhow::Result;

use super::TIMEOUT;
use super::http::{Http, urlencoding_encode};

#[derive(Clone, Copy)]
pub(super) enum Creation<'a> {
    Soul,
    Strand(&'a str),
}

pub(super) async fn post(
    client: &reqwest::Client,
    url: &str,
    body: Option<serde_json::Value>,
    creation: Creation<'_>,
) -> Result<serde_json::Value> {
    let mut call = client.post(url).timeout(TIMEOUT);
    if let Some(body) = body {
        call = call.json(&body);
    }
    let response = call
        .send()
        .await
        .map_err(|error| creation.unknown(format!("POST {url} returned no response: {error}")))?;
    let status = response.status();
    let body = response.text().await.map_err(|error| {
        creation.unknown(format!("read response body with status {status}: {error}"))
    })?;
    if status.is_client_error() {
        return Err(creation.refused(format!(
            "POST {url} returned status {status}: {}",
            crate::watch::snippet(&body, 500)
        )));
    }
    if !status.is_success() {
        return Err(creation.unknown(format!(
            "POST {url} returned ambiguous status {status}: {}",
            crate::watch::snippet(&body, 500)
        )));
    }
    serde_json::from_str(&body)
        .map_err(|error| creation.unknown(format!("successful response was not JSON: {error}")))
}

pub(super) fn identity<'a>(
    creation: Creation<'_>,
    value: &'a serde_json::Value,
    path: &str,
) -> Result<&'a str> {
    value
        .pointer(path)
        .and_then(serde_json::Value::as_str)
        .filter(|held| !held.is_empty() && held.trim() == *held)
        .ok_or_else(|| creation.unknown(format!("successful response missing valid {path}")))
}

impl Creation<'_> {
    pub(super) fn mismatch(self, detail: String) -> anyhow::Error {
        match self {
            Self::Soul => anyhow::anyhow!("tui soul creation outcome=applied_mismatch: {detail}"),
            Self::Strand(soul) => anyhow::anyhow!(
                "tui strand creation outcome=applied_mismatch: {detail}; preserve known soul as `SANTI_SOUL_ID={soul}` and do not create another strand"
            ),
        }
    }

    fn refused(self, detail: String) -> anyhow::Error {
        match self {
            Self::Soul => anyhow::anyhow!("tui soul creation outcome=not_created: {detail}"),
            Self::Strand(soul) => anyhow::anyhow!(
                "tui strand creation outcome=not_created: {detail}; known soul remains `SANTI_SOUL_ID={soul}`"
            ),
        }
    }

    fn unknown(self, detail: String) -> anyhow::Error {
        match self {
            Self::Soul => anyhow::anyhow!(
                "tui soul creation outcome=state_unknown: {detail}; the server may have committed a soul but its server-assigned identity is unknown; do not retry or restart cold-start creation; inspect `GET /api/v1/souls` for an identity whose requested initial memory was published, then resume without --memory-file using `SANTI_SOUL_ID=<recovered-soul> santi tui`"
            ),
            Self::Strand(soul) => anyhow::anyhow!(
                "tui strand creation outcome=state_unknown: {detail}; the server may have committed a strand but its server-assigned identity is unknown; known soul {soul} remains recoverable; do not retry or restart strand creation; preserve `SANTI_SOUL_ID={soul}`, inspect `santi strand list`, then resume with `SANTI_SOUL_ID={soul} SANTI_STRAND_ID=<recovered-strand> santi tui`"
            ),
        }
    }
}

impl Http<'_> {
    pub(super) async fn create_strand(&self, base: &str, soul: Option<&str>) -> Result<()> {
        let url = route(base, soul);
        let response = self
            .client
            .post(&url)
            .timeout(TIMEOUT)
            .send()
            .await
            .map_err(|error| unknown(soul, format!("POST {url} returned no response: {error}")))?;
        let status = response.status();
        let body = response.text().await.map_err(|error| {
            unknown(
                soul,
                format!(
                    "response body from POST {url} with status {status} was unreadable: {error}"
                ),
            )
        })?;
        let created = decode(status, &body, soul)?;
        println!("{}", serde_json::to_string_pretty(&created)?);
        Ok(())
    }
}

fn route(base: &str, soul: Option<&str>) -> String {
    match soul {
        Some(soul) => format!("{base}/api/v1/souls/{}/strands", urlencoding_encode(soul)),
        None => format!("{base}/api/v1/strands"),
    }
}

fn decode(
    status: reqwest::StatusCode,
    body: &str,
    expected_soul: Option<&str>,
) -> Result<serde_json::Value> {
    if status.is_client_error() {
        return Err(anyhow::anyhow!(
            "strand create outcome=not_created: request failed with status {status}: {}",
            crate::watch::snippet(body, 500)
        ));
    }
    if !status.is_success() {
        return Err(unknown(
            expected_soul,
            format!(
                "request returned ambiguous status {status}: {}",
                crate::watch::snippet(body, 500)
            ),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(body).map_err(|error| {
        unknown(
            expected_soul,
            format!("successful response was not JSON: {error}"),
        )
    })?;
    let id = text(&value, "/strand/id", expected_soul)?;
    let owner = text(&value, "/strand/soul", expected_soul)?;
    if let Some(expected_soul) = expected_soul
        && owner != expected_soul
    {
        anyhow::bail!(
            "strand create outcome=applied_mismatch: created strand {id} belongs to {owner}, expected {expected_soul}"
        );
    }
    Ok(value)
}

fn text<'a>(value: &'a serde_json::Value, path: &str, soul: Option<&str>) -> Result<&'a str> {
    value
        .pointer(path)
        .and_then(serde_json::Value::as_str)
        .filter(|held| !held.is_empty() && held.trim() == *held)
        .ok_or_else(|| unknown(soul, format!("successful response missing valid {path}")))
}

fn unknown(soul: Option<&str>, detail: String) -> anyhow::Error {
    let parent = soul.map_or_else(
        || "the runtime-default parent soul remains server-owned".to_string(),
        |soul| format!("preserve known parent soul as `SANTI_SOUL_ID={soul}`"),
    );
    anyhow::anyhow!(
        "strand create outcome=state_unknown: {detail}; the server may have committed a strand but its server-assigned identity is unknown; {parent}; do not retry creation; inspect `santi strand list`, then resume with `SANTI_STRAND_ID=<recovered-strand> santi strand get`"
    )
}
