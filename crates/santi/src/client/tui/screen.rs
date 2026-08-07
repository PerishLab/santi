use std::io::{IsTerminal, Write};

use anyhow::Result;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use super::keys;
use super::parse::{lines, number};
use super::state::{Beat, State, Step};
use super::{Identity, Request, paint};
use crate::client::send::{Request as Send, Target, emission};

pub(super) fn available() -> bool {
    std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
}

struct Sink(UnboundedSender<Beat>);

impl Write for Sink {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buffer).into_owned();
        let _ = self.0.send(Beat::Text(text));
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(super) async fn run(request: Request<'_>) -> Result<()> {
    let identity = request.identify().await?;
    let mut state = State::new(identity.soul.clone(), identity.strand.clone());
    let (omitted, spoken) = lines(identity.detail.as_ref(), usize::MAX);
    state.seed(omitted, header(&identity, spoken));
    state.context = context(&request, &identity).await;
    let outcome = drive(&request, &identity, &mut state).await;
    ratatui::restore();
    outcome
}

async fn drive(request: &Request<'_>, identity: &Identity, state: &mut State) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut strokes = keys::listen();
    let (sender, mut beats) = unbounded_channel();
    let target = Target::tui(request.client, request.base, &identity.strand);
    let mut active = None;
    loop {
        terminal.draw(|frame| paint::draw(frame, state))?;
        let step = tokio::select! {
            stroke = strokes.recv() => state.struck(stroke),
            beat = beats.recv() => state.heard(beat),
            () = poll(&mut active) => Step::Idle,
        };
        match step {
            Step::Leave => return Ok(()),
            Step::Idle => active = None,
            Step::Refresh => state.context = context(request, identity).await,
            Step::Speak(text) => {
                active = Some(Box::pin(dispatch(target, text, sender.clone())));
            }
            Step::Stay => {}
        }
    }
}

fn header(identity: &Identity, spoken: Vec<String>) -> Vec<String> {
    let mut out = vec![
        format!("soul: {}", identity.soul),
        format!("strand: {}", identity.strand),
        "commands: /status /exit · PageUp and PageDown scroll".to_string(),
    ];
    out.extend(spoken);
    out
}

async fn poll(active: &mut Option<std::pin::Pin<Box<impl Future<Output = ()>>>>) {
    match active.as_mut() {
        Some(running) => running.await,
        None => std::future::pending().await,
    }
}

async fn dispatch(target: Target<'_>, text: String, sender: UnboundedSender<Beat>) {
    let mut sink = Sink(sender.clone());
    let body = serde_json::json!({ "content": [{ "type": "text", "text": text }] });
    let outcome = emission(
        Send {
            target,
            body,
            watch: true,
        },
        &mut sink,
    )
    .await;
    let beat = match outcome {
        Ok(Some(done)) => Beat::Settled(done.0),
        Ok(None) => Beat::Broken("send returned without a receipt proof".to_string()),
        Err(error) => Beat::Broken(format!("{error:#}")),
    };
    let _ = sender.send(beat);
}

async fn context(request: &Request<'_>, identity: &Identity) -> String {
    let path = format!("/api/v1/strands/{}/budget", identity.strand);
    let Ok(value) = request.get(&path).await else {
        return "context: unread".to_string();
    };
    let Ok(total) = number(&value, "/estimate/total") else {
        return "context: unread".to_string();
    };
    match value
        .pointer("/budget/bytes")
        .and_then(serde_json::Value::as_i64)
    {
        Some(cap) => format!("context: {total}/{cap} bytes"),
        None => format!("context: {total} bytes (unbounded)"),
    }
}
