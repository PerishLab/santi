use crate::cli::WatchFormat;
use crate::client::{Proof, Target, prove, uncertain};
use anyhow::{Context, Result};
use bound::{SILENCE, boundary, ceiling};
use futures_util::{Stream, StreamExt};
use std::collections::HashMap;
use std::io::Write;
use tokio::time::Instant;

tokio::task_local! {
    static HARNESS: Harness;
}
#[doc(hidden)]
pub struct Harness {
    pub started: tokio::sync::watch::Sender<Instant>,
    pub silence: tokio::sync::watch::Sender<Instant>,
}
pub(crate) struct Watch<'a> {
    pub(crate) target: Target<'a>,
    response: reqwest::Response,
    started: Instant,
}
#[derive(Clone, Copy)]
pub(crate) enum Presentation {
    Watch(WatchFormat),
    Tui,
}
struct Display {
    presentation: Presentation,
    speaking: bool,
    newline: bool,
    spoken: HashMap<String, String>,
    gap: bool,
}
pub(crate) async fn subscribe(target: Target<'_>) -> Result<Watch<'_>> {
    let url = format!("{}/api/v1/strands/{}/events", target.base, target.strand);
    let started = Instant::now();
    let response = tokio::time::timeout_at(started + SILENCE, target.client.get(&url).send())
        .await
        .map_err(|_| {
            anyhow::anyhow!("GET {url} did not establish an event stream in sixty seconds")
        })?
        .with_context(|| format!("GET {url}"))?;
    if !response.status().is_success() {
        anyhow::bail!("request failed with status {}", response.status());
    }
    let _ = HARNESS.try_with(|harness| harness.started.send(started));
    Ok(Watch {
        target,
        response,
        started,
    })
}
pub(crate) async fn follow(
    watch: Watch<'_>,
    receipt: String,
    output: &mut impl Write,
) -> Result<()> {
    let Watch {
        target,
        response,
        started,
    } = watch;
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut display = Display::new(target.presentation);
    let limit = ceiling(target.presentation, started);
    let mut silence = started + SILENCE;
    loop {
        let frame = match tokio::time::timeout_at(
            limit.map_or(silence, |limit| limit.min(silence)),
            next_sse_frame(&mut stream, &mut buffer),
        )
        .await
        {
            Ok(Ok(frame)) => frame,
            Ok(Err(error)) => {
                display.finish(output);
                return Err(uncertain(
                    target,
                    &receipt,
                    format!("event stream failed: {error:#}"),
                ));
            }
            Err(_) => {
                display.finish(output);
                settle(target, &receipt, Some(boundary(started, limit))).await?;
                return Ok(());
            }
        };
        let Some((event, data)) = frame else {
            display.finish(output);
            settle(target, &receipt, Some("event stream ended")).await?;
            return Ok(());
        };
        silence = Instant::now() + SILENCE;
        let _ = HARNESS.try_with(|harness| harness.silence.send(silence));
        display.write(output, &event, &data);
        if terminal(&event, &data) && settle(target, &receipt, None).await? {
            display.finish(output);
            return Ok(());
        }
    }
}
async fn settle(target: Target<'_>, receipt: &str, boundary: Option<&str>) -> Result<bool> {
    match prove(target, receipt).await? {
        Proof::Completed => Ok(true),
        Proof::Failed => anyhow::bail!(
            "strand send outcome=failed: accepted receipt {receipt} reached its durable failed state; do not resend the accepted message; inspect with `santi receipt {receipt}`"
        ),
        Proof::Pending(state) if boundary.is_some() => anyhow::bail!(
            "watch outcome=state_unknown: {} while accepted receipt {receipt} remained {state}; do not resend the accepted message; inspect with `santi receipt {receipt}` and `santi strand runtime {}`; resume the blocking condition or explicitly redrive with `santi strand drive {}`",
            boundary.expect("pending boundary"),
            target.strand,
            target.strand
        ),
        Proof::Pending(_) => Ok(false),
    }
}
impl Display {
    fn new(presentation: Presentation) -> Self {
        Self {
            presentation,
            speaking: false,
            newline: false,
            spoken: HashMap::new(),
            gap: false,
        }
    }
    fn write(&mut self, output: &mut impl Write, event: &str, data: &str) {
        if event == "gap" {
            self.gap = true;
            self.finish(output);
        }
        match self.presentation {
            Presentation::Watch(WatchFormat::Raw) => {
                if event != "open" {
                    writeln!(output, "{data}").ok();
                    output.flush().ok();
                }
            }
            Presentation::Watch(WatchFormat::Filtered) => line(output, event, data),
            Presentation::Tui => self.tui(output, event, data),
        }
    }
    fn tui(&mut self, output: &mut impl Write, event: &str, data: &str) {
        let beat = json_field(data, &["payload", "beat"]);
        if event == "message" && beat.as_deref() == Some("delta") {
            let Some(text) = json_field(data, &["payload", "text"]) else {
                return;
            };
            let text = speech(&text);
            if text.is_empty() {
                return;
            }
            if !self.speaking {
                write!(output, "soul> ").ok();
                self.speaking = true;
            }
            write!(output, "{text}").ok();
            self.newline = text.ends_with('\n');
            if let Some(turn) = json_field(data, &["payload", "turn"]) {
                self.spoken.entry(turn).or_default().push_str(&text);
            }
            output.flush().ok();
            return;
        }
        self.finish(output);
        let completed = event == "message" && beat.as_deref() == Some("completed");
        let duplicate = completed
            && !self.gap
            && json_field(data, &["payload", "turn"]).is_some_and(|turn| {
                json_field(data, &["payload", "message", "text"])
                    .map(|text| speech(&text))
                    .is_some_and(|text| self.spoken.get(&turn) == Some(&text))
            });
        if !duplicate {
            line(output, event, data);
        }
        if event == "turn"
            && matches!(beat.as_deref(), Some("completed") | Some("failed"))
            && let Some(turn) = json_field(data, &["payload", "turn"])
        {
            self.spoken.remove(&turn);
        }
    }
    fn finish(&mut self, output: &mut impl Write) {
        if self.speaking && !self.newline {
            writeln!(output).ok();
            output.flush().ok();
        }
        self.speaking = false;
        self.newline = false;
    }
}
fn line(output: &mut impl Write, event: &str, data: &str) {
    if let Some(line) = render_watch_event(event, data) {
        writeln!(output, "{line}").ok();
        output.flush().ok();
    }
}
fn speech(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}
fn terminal(event: &str, data: &str) -> bool {
    let beat = json_field(data, &["payload", "beat"]);
    event == "turn" && matches!(beat.as_deref(), Some("completed") | Some("failed"))
}
mod bound;
mod render;
pub use render::*;
pub fn snippet(text: &str, limit: usize) -> String {
    let plain = text
        .chars()
        .map(|character| match character {
            character if character.is_control() => ' ',
            character => character,
        })
        .collect::<String>();
    let normalized = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out = normalized.chars().take(limit).collect::<String>();
    if normalized.chars().count() > limit {
        out.push('…');
    }
    out
}
#[doc(hidden)]
pub async fn harness<T>(support: Harness, future: impl std::future::Future<Output = T>) -> T {
    HARNESS.scope(support, future).await
}
pub async fn next_sse_frame<B: AsRef<[u8]>>(
    stream: &mut (impl Stream<Item = reqwest::Result<B>> + Unpin),
    buffer: &mut Vec<u8>,
) -> Result<Option<(String, String)>> {
    loop {
        while let Some(end) = delimiter(buffer) {
            let frame = buffer.drain(..end).collect::<Vec<_>>();
            let frame =
                std::str::from_utf8(&frame).context("decode complete SSE frame as UTF-8")?;
            if let Some(parsed) = parse_sse_frame(frame) {
                return Ok(Some(parsed));
            }
            if !frame
                .lines()
                .all(|line| line.is_empty() || line.starts_with(':'))
            {
                anyhow::bail!("invalid SSE frame did not contain an event field");
            }
        }
        match stream.next().await {
            Some(chunk) => {
                let chunk = chunk.context("read event stream")?;
                buffer.extend_from_slice(chunk.as_ref());
            }
            None if buffer.is_empty() => return Ok(None),
            None => anyhow::bail!("event stream ended with an incomplete SSE frame"),
        }
    }
}
fn delimiter(buffer: &[u8]) -> Option<usize> {
    [b"\n\n".as_slice(), b"\r\n\r\n".as_slice()]
        .into_iter()
        .filter_map(|marker| {
            buffer
                .windows(marker.len())
                .position(|window| window == marker)
                .map(|position| position + marker.len())
        })
        .min()
}
pub fn parse_sse_frame(frame: &str) -> Option<(String, String)> {
    let mut event = None;
    let mut data = String::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    event.map(|event| (event, data))
}
pub fn json_field(data: &str, path: &[&str]) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(data).ok()?;
    let value = path.iter().try_fold(&value, |value, key| value.get(*key))?;
    value.as_str().map(str::to_string)
}
