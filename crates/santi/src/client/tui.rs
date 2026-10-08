use std::fs;
use std::io::{BufRead, Write};
use std::time::Duration;

use anyhow::{Context, Result};

use super::create::{Creation, identity, post};
use super::send::{Request as Send, Target, emission};
use crate::cli::ClientDefaults;

pub(super) use crate::watch::Kind;

const TIMEOUT: Duration = Duration::from_secs(30);
const HISTORY: usize = 20;

pub struct Request<'a> {
    pub client: &'a reqwest::Client,
    pub base: &'a str,
    pub defaults: &'a ClientDefaults,
    pub bearer: Option<&'a str>,
    pub memory: Option<String>,
}

pub(super) struct Identity {
    pub(super) soul: String,
    pub(super) strand: String,
    pub(super) detail: Option<serde_json::Value>,
}

pub async fn run(request: Request<'_>) -> Result<()> {
    if screen::available() {
        return screen::run(request).await;
    }
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    session(request, &mut input, &mut output).await
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
    writeln!(
        output,
        "commands: /status /jobs /job N [stdout|stderr] [cursor] /exit"
    )?;
    let mut catalog = super::jobs::Catalog::default();

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
            "/reload" => {
                writeln!(
                    output,
                    "reload refused: /reload requires an interactive terminal"
                )?;
                continue;
            }
            "/status" => {
                request.status(&identity, output).await?;
                continue;
            }
            _ => {}
        }
        if super::jobs::command(text) {
            let (held, report) =
                super::jobs::inspect(&request, &identity, catalog, text.to_string()).await;
            catalog = held;
            writeln!(output, "{report}")?;
            continue;
        }
        let completed = emission(
            Send {
                target: Target::tui(request.client, request.base, &identity.strand),
                body: serde_json::json!({
                    "content": [{ "type": "text", "text": text }]
                }),
                watch: true,
            },
            &mut crate::watch::Bytes(&mut *output),
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
    pub(super) async fn identify(&self) -> Result<Identity> {
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

    pub(super) async fn get(&self, path: &str) -> Result<serde_json::Value> {
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

mod keys;
mod layout;
mod paint;
mod parse;
mod reload;
mod screen;
mod state;

use parse::{decode, lines, number, text};

fn history(output: &mut impl Write, detail: Option<&serde_json::Value>) -> Result<()> {
    let (omitted, spoken) = lines(detail, HISTORY);
    if omitted > 0 {
        writeln!(output, "history: {omitted} earlier messages omitted")?;
    }
    for held in spoken {
        for line in held.lines {
            writeln!(output, "{}> {line}", held.who)?;
        }
    }
    Ok(())
}
