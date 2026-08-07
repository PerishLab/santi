use async_stream::stream;
use futures_core::Stream;
use futures_util::StreamExt;
use reqwest::{Client, Response};
use serde_json::Value;

use crate::{Event, Trace};

use super::stream::frames;

pub(super) struct Attempt {
    pub(super) client: Client,
    pub(super) url: String,
    pub(super) key: String,
    pub(super) body: Value,
    pub(super) rounds: u32,
}

enum Beat {
    Passed(Event),
    Held(Event),
    Spoken(Event),
    Reported(String),
    Torn(String),
}

enum Failure {
    Reported(String),
    Torn(String),
}

impl Failure {
    fn detail(&self) -> String {
        match self {
            Self::Reported(detail) | Self::Torn(detail) => detail.clone(),
        }
    }

    fn resent(self) -> Result<Event, String> {
        match self {
            Self::Reported(detail) => Ok(Event::Failed(detail)),
            Self::Torn(detail) => Err(detail),
        }
    }
}

struct Gate {
    held: Vec<Event>,
    open: bool,
    failure: Option<Failure>,
}

impl Gate {
    fn new() -> Self {
        Self {
            held: Vec::new(),
            open: false,
            failure: None,
        }
    }

    fn absorb(&mut self, beat: Beat) -> Vec<Event> {
        match beat {
            Beat::Passed(event) => vec![event],
            Beat::Held(event) if self.open => vec![event],
            Beat::Held(event) => {
                self.held.push(event);
                Vec::new()
            }
            Beat::Spoken(event) => self.released(event),
            Beat::Reported(detail) => self.broke(Failure::Reported(detail)),
            Beat::Torn(detail) => self.broke(Failure::Torn(detail)),
        }
    }

    fn released(&mut self, event: Event) -> Vec<Event> {
        self.open = true;
        let mut out = std::mem::take(&mut self.held);
        out.push(event);
        out
    }

    fn broke(&mut self, failure: Failure) -> Vec<Event> {
        self.failure = Some(failure);
        Vec::new()
    }
}

fn sorted(item: Result<Event, String>) -> Beat {
    match item {
        Err(detail) => Beat::Torn(detail),
        Ok(Event::Failed(detail)) => Beat::Reported(detail),
        Ok(event @ Event::Traced(_)) => Beat::Passed(event),
        Ok(event @ (Event::Started { .. } | Event::Working { .. })) => Beat::Held(event),
        Ok(event) => Beat::Spoken(event),
    }
}

async fn opened(attempt: &Attempt) -> Result<Response, String> {
    let response = attempt
        .client
        .post(&attempt.url)
        .bearer_auth(&attempt.key)
        .json(&attempt.body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    Err(format!("openai responses retry failed: {status} {body}"))
}

pub(super) fn retried(
    attempt: Attempt,
    first: Response,
) -> impl Stream<Item = Result<Event, String>> + Send + 'static {
    stream! {
        let mut inner = Box::pin(frames(first.bytes_stream()));
        let mut gate = Gate::new();
        let mut round = 1;
        loop {
            let Some(item) = inner.next().await else {
                return;
            };
            for event in gate.absorb(sorted(item)) {
                yield Ok(event);
            }
            let Some(failure) = gate.failure.take() else {
                continue;
            };
            if gate.open || round >= attempt.rounds {
                yield failure.resent();
                return;
            }
            yield Ok(Event::Traced(Trace::Retried {
                attempt: round,
                detail: failure.detail(),
            }));
            round += 1;
            let Ok(next) = opened(&attempt).await else {
                yield failure.resent();
                return;
            };
            inner = Box::pin(frames(next.bytes_stream()));
            gate = Gate::new();
        }
    }
}
