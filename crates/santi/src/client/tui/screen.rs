use std::future::Future;
use std::io::{IsTerminal, Write};
use std::process::Command;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

#[path = "screen/naming.rs"]
mod naming;

use super::keys;
use super::parse::{lines, read};
use super::state::recover::recover;
use super::state::{Beat, State, Step};
use super::{Identity, Request, paint};
use crate::client::send::{Request as Send, Target, emission};
use crate::watch::{Emit, Shown};
use naming::{listing, renamed};

const TIMEOUT: Duration = Duration::from_secs(3);

pub(super) fn available() -> bool {
    std::io::stdout().is_terminal() && std::io::stdin().is_terminal()
}

struct Sink(UnboundedSender<Beat>);

impl Write for Sink {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let text = String::from_utf8_lossy(buffer).into_owned();
        let _ = self.0.send(Beat::Speech(text));
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Emit for Sink {
    fn open(&mut self, who: &str) {
        let _ = self.0.send(Beat::Open(who.to_string()));
    }

    fn speech(&mut self, text: &str) {
        let _ = self.0.send(Beat::Speech(text.to_string()));
    }

    fn event(&mut self, shown: Shown<'_>) {
        let _ = self.0.send(Beat::Event(
            shown.kind,
            shown.turn.map(str::to_string),
            shown.line.to_string(),
        ));
    }
}

pub(super) async fn run(request: Request<'_>) -> Result<()> {
    let mut wanted: Option<String> = None;
    loop {
        let identity = match wanted.take() {
            Some(strand) => request.resume(&strand).await?,
            None => request.identify().await?,
        };
        let reload = super::reload::Plan::new(&request, &identity)?;
        let names = crate::config::names().alias;
        let mut state = State::new(identity.soul.clone(), identity.strand.clone(), names);
        let (omitted, spoken) = lines(identity.detail.as_ref(), usize::MAX);
        state.seed(omitted, header(&identity), spoken);
        state.context = "context: refreshing".to_string();
        let outcome = drive(&request, &identity, &reload, &mut state).await;
        ratatui::restore();
        match outcome? {
            Some(next) => wanted = Some(next),
            None => return Ok(()),
        }
    }
}

async fn drive(
    request: &Request<'_>,
    identity: &Identity,
    reload: &super::reload::Plan,
    state: &mut State,
) -> Result<Option<String>> {
    let mut terminal = ratatui::init();
    let mut strokes = keys::listen();
    let (sender, mut beats) = unbounded_channel();
    let target = Target::tui(request.client, request.base, &identity.strand);
    let mut active = None;
    let mut listening = Some(Box::pin(resident(target, sender.clone())));
    let mut refresh = Some(Box::pin(context(request, identity)));
    let mut stopping = None;
    let mut pulse = tokio::time::interval(Duration::from_millis(120));
    let mut cache = paint::Cache::new();
    loop {
        terminal.draw(|frame| paint::draw(frame, state, &mut cache))?;
        let step = tokio::select! {
            stroke = strokes.recv() => state.struck(stroke),
            beat = beats.recv() => state.heard(beat),
            () = poll(&mut active) => Step::Idle,
            never = poll(&mut listening) => match never {},
            value = poll(&mut refresh) => {
                state.context = value;
                refresh = None;
                Step::Stay
            },
            detail = poll(&mut stopping) => {
                state.push(detail);
                stopping = None;
                Step::Stay
            },
            _ = pulse.tick(), if state.since.is_some() => {
                state.tick();
                Step::Stay
            },
        };
        match step {
            Step::Leave => return Ok(None),
            Step::Reload => {
                let prepared = reload.prepare();
                drop(terminal);
                let error = attempted(reload, prepared);
                terminal = recover(state, error, ratatui::try_init, ratatui::restore)?;
            }
            Step::Idle => {
                active = None;
                listening = Some(Box::pin(resident(target, sender.clone())));
            }
            Step::Refresh => {
                state.context = "context: refreshing".to_string();
                refresh = Some(Box::pin(context(request, identity)));
            }
            Step::Speak(text) => {
                listening = None;
                active = Some(Box::pin(dispatch(target, text, sender.clone())));
            }
            Step::Switch(next) => return Ok(Some(next)),
            Step::Listing(kind) => {
                let report = listing(request, &kind).await;
                state.push(report);
            }
            Step::Name(id, name) => {
                let report = renamed(&id, &name);
                state.push(report);
            }
            Step::Stop(turn) => {
                stopping = Some(Box::pin(interrupt(target, turn)));
            }
            Step::Copy(text) => {
                let report = copied(&text);
                state.push(report);
            }
            Step::Stay => {}
        }
    }
}

async fn interrupt(target: Target<'_>, turn: String) -> String {
    let url = format!("{}/api/v1/turns/{turn}/stop", target.base);
    match target.client.post(&url).timeout(TIMEOUT).send().await {
        Ok(response) if response.status().is_success() => {
            format!("interrupt accepted for turn {turn}; awaiting its durable outcome")
        }
        Ok(response) => format!(
            "interrupt refused for turn {turn} with status {}; nothing was changed",
            response.status()
        ),
        Err(error) => format!(
            "interrupt for turn {turn} could not be delivered: {error}; the turn may still be \
             running"
        ),
    }
}

fn attempted(reload: &super::reload::Plan, prepared: Result<Command>) -> anyhow::Error {
    match prepared {
        Ok(command) => reload.exec(command),
        Err(error) => error,
    }
}

fn carried(outcome: Result<()>) -> String {
    match outcome {
        Ok(()) => "the event stream ended".to_string(),
        Err(error) => format!("{error:#}"),
    }
}

fn copied(text: &str) -> String {
    match copy(text) {
        Ok(()) => format!("copied {} bytes to the terminal clipboard", text.len()),
        Err(error) => format!("copy failed: {error}"),
    }
}

fn copy(text: &str) -> std::io::Result<()> {
    let mut out = std::io::stdout();
    out.write_all(super::paint::clip::osc52(text).as_bytes())?;
    out.flush()
}

fn header(identity: &Identity) -> Vec<String> {
    vec![
        format!("soul {} · strand {}", identity.soul, identity.strand),
        "/status  /reload  /exit".to_string(),
    ]
}

async fn poll<T>(active: &mut Option<std::pin::Pin<Box<impl Future<Output = T>>>>) -> T {
    match active.as_mut() {
        Some(running) => running.await,
        None => std::future::pending().await,
    }
}

async fn resident(target: Target<'_>, sender: UnboundedSender<Beat>) -> std::convert::Infallible {
    const FLOOR: Duration = Duration::from_secs(1);
    const CEILING: Duration = Duration::from_secs(30);
    let mut backoff = FLOOR;
    loop {
        let mut sink = Sink(sender.clone());
        let detail = match crate::watch::subscribe(target).await {
            Ok(watch) => {
                let _ = sender.send(Beat::Live);
                backoff = FLOOR;
                carried(crate::watch::observe(watch, &mut sink).await)
            }
            Err(error) => format!("{error:#}"),
        };
        let _ = sender.send(Beat::Lost(detail));
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(CEILING);
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
        Ok(None) => Beat::Unsettled("send returned without a receipt proof".to_string()),
        Err(error)
            if error
                .downcast_ref::<crate::client::send::Unsettled>()
                .is_some() =>
        {
            Beat::Unsettled(format!("{error:#}"))
        }
        Err(error) => Beat::Broken(format!("{error:#}")),
    };
    let _ = sender.send(beat);
}

async fn context(request: &Request<'_>, identity: &Identity) -> String {
    let path = format!("/api/v1/strands/{}/budget", identity.strand);
    read(request.get(&path), TIMEOUT).await
}
