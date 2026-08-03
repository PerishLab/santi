use std::fs;
use std::io::{BufRead, Write};
use std::time::Duration;

use anyhow::{Context, Result};

use super::create::{Creation, identity, post};
use super::send::{Request as Send, Target, emission};
use crate::cli::ClientDefaults;
use crate::watch::snippet;

const TIMEOUT: Duration = Duration::from_secs(30);
const HISTORY: usize = 20;

pub struct Request<'a> {
    pub client: &'a reqwest::Client,
    pub base: &'a str,
    pub defaults: &'a ClientDefaults,
    pub memory: Option<String>,
}

struct Identity {
    soul: String,
    strand: String,
    detail: Option<serde_json::Value>,
}

pub async fn run(
    client: &reqwest::Client,
    base: &str,
    defaults: &ClientDefaults,
    memory: Option<String>,
) -> Result<()> {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    session(
        Request {
            client,
            base,
            defaults,
            memory,
        },
        &mut input,
        &mut output,
    )
    .await
}

pub async fn session(
    request: Request<'_>,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<()> {
    let identity = request.identify().await?;
    writeln!(output, "santi tui")?;
    writeln!(output, "soul: {}", identity.soul)?;
    writeln!(output, "strand: {}", identity.strand)?;
    writeln!(
        output,
        "resume: SANTI_SOUL_ID={} SANTI_STRAND_ID={} santi tui",
        identity.soul, identity.strand
    )?;
    history(output, identity.detail.as_ref())?;
    request.status(&identity, output).await?;
    writeln!(output, "commands: /status /exit")?;

    loop {
        write!(output, "you> ")?;
        output.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            break;
        }
        let text = line.trim();
        match text {
            "" => continue,
            "/exit" => break,
            "/status" => {
                request.status(&identity, output).await?;
                continue;
            }
            _ => {}
        }
        let completed = emission(
            Send {
                target: Target::tui(request.client, request.base, &identity.strand),
                body: serde_json::json!({
                    "content": [{ "type": "text", "text": text }]
                }),
                watch: true,
            },
            output,
        )
        .await?
        .expect("watched send returns completed receipt proof");
        writeln!(
            output,
            "send completed: receipt {} is durably completed; do not resend",
            completed.0
        )?;
        if let Err(error) = request.status(&identity, output).await {
            writeln!(
                output,
                "auxiliary status refresh failed after completed receipt {}; do not resend: {error}",
                completed.0
            )?;
        }
    }
    Ok(())
}

impl Request<'_> {
    async fn identify(&self) -> Result<Identity> {
        let explicit = self
            .defaults
            .strand
            .as_deref()
            .map(str::trim)
            .filter(|strand| !strand.is_empty());
        if self.memory.is_some() && (self.defaults.soul().is_some() || explicit.is_some()) {
            anyhow::bail!("--memory-file only applies when tui awakens a new soul");
        }
        if let Some(strand) = explicit {
            return self.resume(strand).await;
        }
        let soul = match self.defaults.soul() {
            Some(soul) => {
                self.get_soul(soul).await?;
                soul.to_string()
            }
            None => self.awaken().await?,
        };
        let creation = Creation::Strand(&soul);
        let url = format!("{}/api/v1/souls/{soul}/strands", self.base);
        let created = post(self.client, &url, None, creation).await?;
        let strand = identity(creation, &created, "/strand/id")?;
        let owner = identity(creation, &created, "/strand/soul")?;
        if owner != soul {
            return Err(creation.mismatch(format!(
                "created strand {strand} belongs to soul {owner}, expected {soul}"
            )));
        }
        Ok(Identity {
            soul,
            strand: strand.to_string(),
            detail: None,
        })
    }

    async fn resume(&self, strand: &str) -> Result<Identity> {
        let detail = self.get(&format!("/api/v1/strands/{strand}")).await?;
        let resolved = text(&detail, "/strand/id")?;
        if resolved != strand {
            anyhow::bail!("requested strand {strand} resolved as {resolved}");
        }
        let owner = text(&detail, "/strand/soul")?;
        let soul = self.defaults.soul().unwrap_or(owner);
        if owner != soul {
            anyhow::bail!("strand {strand} belongs to soul {owner}, not {soul}");
        }
        self.get_soul(soul).await?;
        Ok(Identity {
            soul: soul.to_string(),
            strand: strand.to_string(),
            detail: Some(detail),
        })
    }

    async fn get_soul(&self, soul: &str) -> Result<()> {
        let detail = self.get(&format!("/api/v1/souls/{soul}")).await?;
        let resolved = text(&detail, "/id")?;
        if resolved != soul {
            anyhow::bail!("requested soul {soul} resolved as {resolved}");
        }
        Ok(())
    }

    async fn awaken(&self) -> Result<String> {
        let memory = self
            .memory
            .as_deref()
            .map(fs::read_to_string)
            .transpose()
            .with_context(|| {
                format!(
                    "read soul memory {}",
                    self.memory.as_deref().unwrap_or_default()
                )
            })?;
        let creation = Creation::Soul;
        let url = format!("{}/api/v1/souls", self.base);
        let soul = post(
            self.client,
            &url,
            Some(serde_json::json!({ "memory": memory })),
            creation,
        )
        .await?;
        Ok(identity(creation, &soul, "/id")?.to_string())
    }

    async fn status(&self, identity: &Identity, output: &mut impl Write) -> Result<()> {
        let value = self
            .get(&format!("/api/v1/strands/{}/budget", identity.strand))
            .await?;
        let total = number(&value, "/estimate/total")?;
        let cap = value
            .pointer("/budget/bytes")
            .and_then(serde_json::Value::as_i64);
        match cap {
            Some(cap) => writeln!(output, "context: {total}/{cap} bytes")?,
            None => writeln!(output, "context: {total} bytes (unbounded)")?,
        }
        Ok(())
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("{}{path}", self.base);
        let response = self
            .client
            .get(&url)
            .timeout(TIMEOUT)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        decode(response).await
    }
}

fn history(output: &mut impl Write, detail: Option<&serde_json::Value>) -> Result<()> {
    let Some(messages) = detail
        .and_then(|detail| detail.get("messages"))
        .and_then(serde_json::Value::as_array)
    else {
        return Ok(());
    };
    let omitted = messages.len().saturating_sub(HISTORY);
    if omitted > 0 {
        writeln!(output, "history: {omitted} earlier messages omitted")?;
    }
    for placed in messages.iter().skip(omitted) {
        let Some(line) = message(placed) else {
            continue;
        };
        writeln!(output, "{line}")?;
    }
    Ok(())
}

fn message(placed: &serde_json::Value) -> Option<String> {
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

async fn decode(response: reqwest::Response) -> Result<serde_json::Value> {
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

fn text<'a>(value: &'a serde_json::Value, path: &str) -> Result<&'a str> {
    value
        .pointer(path)
        .and_then(serde_json::Value::as_str)
        .filter(|held| !held.is_empty() && held.trim() == *held)
        .ok_or_else(|| anyhow::anyhow!("response missing valid {path}"))
}

fn number(value: &serde_json::Value, path: &str) -> Result<i64> {
    value
        .pointer(path)
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| anyhow::anyhow!("response missing {path}"))
}
