use anyhow::{Context, Result};
use futures_util::{Stream, StreamExt};

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
                let chunk = crate::client::read(chunk).context("read event stream")?;
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
