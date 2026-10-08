use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde_json::Value;

use super::tui::{Identity, Request};

const KEPT: usize = 24;
const LIMIT: usize = 4096;
const TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone, Default)]
pub(super) struct Catalog(Vec<String>);

pub(super) fn command(text: &str) -> bool {
    matches!(text.split_whitespace().next(), Some("/jobs" | "/job"))
}

pub(super) async fn inspect(
    request: &Request<'_>,
    identity: &Identity,
    mut catalog: Catalog,
    command: String,
) -> (Catalog, String) {
    if command == "/jobs" {
        catalog.0.clear();
    }
    let outcome = tokio::time::timeout(
        TIMEOUT,
        Reader { request, identity }.run(&mut catalog, &command),
    )
    .await;
    let report = match outcome {
        Ok(Ok(report)) => report,
        Ok(Err(error)) => format!("job inspection refused: {}", safe(&error.to_string(), 500)),
        Err(_) => "job inspection timed out; retry the read command".to_string(),
    };
    (catalog, report)
}

struct Reader<'a, 'b> {
    request: &'a Request<'b>,
    identity: &'a Identity,
}

impl Reader<'_, '_> {
    async fn run(&self, catalog: &mut Catalog, command: &str) -> Result<String> {
        if command == "/jobs" {
            return self.listing(catalog).await;
        }
        if command.split_whitespace().next() == Some("/jobs") {
            bail!("use /jobs without arguments");
        }
        let mut parts = command.split_whitespace().skip(1);
        let ordinal = parts.next().and_then(|part| part.parse::<usize>().ok());
        let stream = parts.next().unwrap_or("stdout");
        let cursor = parts.next().unwrap_or("0");
        if parts.next().is_some() || !matches!(stream, "stdout" | "stderr") {
            bail!("use /job N [stdout|stderr] [cursor]");
        }
        cursor
            .parse::<u64>()
            .context("cursor must be an unsigned number")?;
        let index = ordinal.and_then(|held| held.checked_sub(1));
        let id = index.and_then(|index| catalog.0.get(index));
        let Some(id) = id else {
            bail!("run /jobs, then select a listed ordinal with /job N");
        };
        let path = format!("/api/v1/jobs/{}", super::urlencoding_encode(id));
        let job = self.get(&path).await?;
        owned(&job, self.identity)?;
        if text(&job, "/id")? != id || text(&job, "/origin/strand")? != self.identity.strand {
            bail!("job response does not match the selected current-strand job");
        }
        let path = format!("{path}/logs?stream={stream}&cursor={cursor}&limit={LIMIT}");
        let log = self.get(&path).await?;
        if text(&log, "/job")? != id || text(&log, "/stream")? != stream {
            bail!("log response does not match the selected job and stream");
        }
        let ordinal = ordinal.expect("catalog selection requires ordinal");
        detail(&job, &log, ordinal)
    }

    async fn listing(&self, catalog: &mut Catalog) -> Result<String> {
        let value = self.get("/api/v1/jobs").await?;
        let jobs = value.as_array().context("job list is not an array")?;
        let mut report = vec!["current-strand jobs:".to_string()];
        let mut count = 0;
        let mut ids = Vec::new();
        for job in jobs {
            owned(job, self.identity)?;
            if text(job, "/origin/strand")? != self.identity.strand {
                continue;
            }
            count += 1;
            if ids.len() == KEPT {
                continue;
            }
            let id = text(job, "/id")?;
            if ids.iter().any(|held| held == id) {
                bail!("job list contains duplicate identities");
            }
            ids.push(id.to_string());
            report.push(format!(
                "{}: {} · {} · {}",
                ids.len(),
                safe(id, 160),
                safe(text(job, "/state")?, 160),
                safe(text(job, "/description")?, 160),
            ));
        }
        if count == 0 {
            report.push("no jobs for this strand".to_string());
        }
        if count > KEPT {
            report.push(format!(
                "{} further jobs omitted; use santi job list",
                count - KEPT
            ));
        }
        report
            .push("inspect: /job N [stdout|stderr] [cursor]; /jobs refreshes ordinals".to_string());
        catalog.0 = ids;
        Ok(report.join("\n"))
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let response = self
            .request
            .client
            .get(format!("{}{path}", self.request.base))
            .header("x-santi-soul-id", &self.identity.soul)
            .timeout(TIMEOUT)
            .send()
            .await
            .context("read job response")?;
        let status = response.status();
        if !status.is_success() {
            bail!("job request failed with status {status}");
        }
        response.json().await.context("decode job response")
    }
}

fn detail(job: &Value, log: &Value, ordinal: usize) -> Result<String> {
    let cursor = text(log, "/cursor")?
        .parse::<u64>()
        .context("invalid log cursor")?;
    let next = text(log, "/next")?
        .parse::<u64>()
        .context("invalid next cursor")?;
    let eof = log
        .get("eof")
        .and_then(Value::as_bool)
        .context("missing log eof")?;
    let data = log
        .get("data")
        .and_then(Value::as_str)
        .context("missing log data")?;
    if next < cursor || next - cursor > LIMIT as u64 || data.chars().count() > LIMIT {
        bail!("log response exceeds the requested chunk boundary");
    }
    let mut report = vec![format!(
        "job {} · {} · exit {}",
        safe(text(job, "/id")?, 160),
        safe(text(job, "/state")?, 160),
        safe(
            &job.get("exit_code").unwrap_or(&Value::Null).to_string(),
            160
        ),
    )];
    report.push(safe(text(job, "/description")?, 160));
    for key in ["soul", "strand", "turn", "call", "effect"] {
        report.push(format!(
            "{key}: {}",
            safe(text(job, &format!("/origin/{key}"))?, 160)
        ));
    }
    let stream = text(log, "/stream")?;
    report.push(format!(
        "{stream} · cursor {cursor} → {next} · current eof {eof}"
    ));
    report.push(safe(data, LIMIT));
    report.push(format!("continue/refresh: /job {ordinal} {stream} {next}"));
    Ok(report.join("\n"))
}

fn owned(job: &Value, identity: &Identity) -> Result<()> {
    if text(job, "/origin/soul")? != identity.soul {
        bail!("job response belongs to another soul");
    }
    Ok(())
}

fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty() && text.trim() == *text)
        .with_context(|| format!("missing valid {path}"))
}

fn safe(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|held| !held.is_control() || matches!(held, '\n' | '\t'))
        .take(limit)
        .collect()
}
