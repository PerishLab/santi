use crate::cli::WatchFormat;
use crate::client::{Proof, Target, prove, uncertain};
use anyhow::{Context, Result};
use bound::{SILENCE, boundary, ceiling};
use futures_util::{Stream, StreamExt};
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Thinking,
    Tool,
    Turn,
    Fault,
    Other,
}

impl Kind {
    fn of(event: &str, beat: &str) -> Self {
        match (event, beat) {
            ("thinking", _) => Self::Thinking,
            ("tool", _) => Self::Tool,
            ("turn", "failed") | ("transition", _) => Self::Fault,
            ("turn", _) => Self::Turn,
            _ => Self::Other,
        }
    }
}

pub(crate) struct Shown<'a> {
    pub(crate) kind: Kind,
    pub(crate) turn: Option<&'a str>,
    pub(crate) line: &'a str,
}

pub(crate) trait Emit: Write {
    fn speech(&mut self, text: &str);
    fn event(&mut self, shown: Shown<'_>);
}

pub struct Bytes<W>(pub W);

impl<W: Write> Write for Bytes<W> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

impl<W: Write> Emit for Bytes<W> {
    fn speech(&mut self, text: &str) {
        write!(self.0, "{text}").ok();
        self.0.flush().ok();
    }

    fn event(&mut self, shown: Shown<'_>) {
        writeln!(self.0, "{}", shown.line).ok();
        self.0.flush().ok();
    }
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
    output: &mut impl Emit,
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
                    target.strand,
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
pub(crate) async fn observe(watch: Watch<'_>, output: &mut impl Emit) -> Result<()> {
    let Watch { response, .. } = watch;
    let mut stream = response.bytes_stream();
    let mut buffer = Vec::new();
    let mut display = Display::new(Presentation::Tui);
    loop {
        match next_sse_frame(&mut stream, &mut buffer).await {
            Ok(Some((event, data))) => display.write(output, &event, &data),
            Ok(None) => {
                display.finish(output);
                return Ok(());
            }
            Err(error) => {
                display.finish(output);
                return Err(error);
            }
        }
    }
}

async fn settle(target: Target<'_>, receipt: &str, boundary: Option<&str>) -> Result<bool> {
    match prove(target, receipt).await? {
        Proof::Completed => Ok(true),
        Proof::Failed => anyhow::bail!(
            "strand send outcome=failed: accepted receipt {receipt} reached its durable failed state; do not resend the accepted message; inspect with `santi receipt {receipt}`"
        ),
        Proof::Pending(state) if boundary.is_some() => Err(crate::client::unsettled(format!(
            "watch outcome=state_unknown: {} while accepted receipt {receipt} remained {state}; do not resend the accepted message; inspect with `santi receipt {receipt}` and `santi strand runtime {}`; resume the blocking condition or explicitly redrive with `santi strand drive {}`",
            boundary.expect("pending boundary"),
            target.strand,
            target.strand
        ))),
        Proof::Pending(_) => Ok(false),
    }
}
fn terminal(event: &str, data: &str) -> bool {
    let beat = json_field(data, &["payload", "beat"]);
    event == "turn" && matches!(beat.as_deref(), Some("completed") | Some("failed"))
}
mod bound;
mod display;
mod render;

use display::Display;
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
