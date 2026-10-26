use crate::{Item, Request};

use super::{Config, Replay};

pub(super) fn prepare(config: &Config, mut request: Request) -> Result<Request, String> {
    if config.replay == Replay::Stateless || config.thinking.as_deref() == Some("disabled") {
        request
            .input
            .retain(|item| !matches!(item, Item::Continuation(_) | Item::Boundary));
        return Ok(request);
    }
    let start = if request
        .tools
        .as_ref()
        .is_some_and(|tools| !tools.is_empty())
    {
        0
    } else {
        request.input.iter().rposition(inbound).unwrap_or(0)
    };
    request.input = request
        .input
        .into_iter()
        .enumerate()
        .filter_map(|(index, item)| {
            if index < start && matches!(item, Item::Continuation(_) | Item::Boundary) {
                None
            } else {
                Some(item)
            }
        })
        .collect();
    Ok(request)
}

pub(super) fn validate(config: &Config, request: &Request, complete: bool) -> Result<(), String> {
    if config.replay == Replay::Stateless || config.thinking.as_deref() == Some("disabled") {
        return Ok(());
    }
    let start = if request
        .tools
        .as_ref()
        .is_some_and(|tools| !tools.is_empty())
    {
        0
    } else {
        request.input.iter().rposition(inbound).unwrap_or(0)
    };
    let mut group = Group::default();
    for item in request.input.iter().skip(start) {
        match item {
            Item::Continuation(state) => {
                group.finish(true)?;
                compatible(config, request, state)?;
                group = Group {
                    expected: Some(state.calls),
                    ..Default::default()
                };
            }
            Item::Boundary => {
                group.finish(true)?;
                group = Group::default();
            }
            item if inbound(item) => {
                group.finish(true)?;
                group = Group::default();
            }
            Item::Message { role, .. } if role == "assistant" && group.expected.is_none() => {
                return Err(missing());
            }
            Item::Call { call, .. } => group.call(call)?,
            Item::Output { call, .. } => group.reply(call)?,
            _ => {}
        }
    }
    group.finish(complete)
}

#[derive(Default)]
struct Group {
    expected: Option<usize>,
    calls: std::collections::BTreeSet<String>,
    replies: std::collections::BTreeSet<String>,
}

impl Group {
    fn call(&mut self, call: &str) -> Result<(), String> {
        if self.expected.is_none() {
            return Err(missing());
        }
        if !self.calls.insert(call.into()) {
            return Err("duplicate call in reasoning continuation".into());
        }
        Ok(())
    }

    fn reply(&mut self, call: &str) -> Result<(), String> {
        if !self.calls.contains(call) || !self.replies.insert(call.into()) {
            return Err("orphan or duplicate result in reasoning continuation; restore a complete tool chain".into());
        }
        Ok(())
    }

    fn finish(&self, complete: bool) -> Result<(), String> {
        let Some(expected) = self.expected else {
            return Ok(());
        };
        if self.calls.len() > expected {
            return Err(incomplete());
        }
        if complete && (self.calls.len() != expected || self.replies.len() != expected) {
            return Err(incomplete());
        }
        Ok(())
    }
}

fn inbound(item: &Item) -> bool {
    matches!(item, Item::Message { role, .. } if role == "user" || role == "system")
}

fn missing() -> String {
    "original reasoning continuation is missing for selected assistant history; compact the completed legacy history or create a new strand before enabling history replay; summaries cannot reconstruct protocol state".into()
}

fn compatible(
    config: &Config,
    request: &Request,
    state: &santi_model::thinking::Continuation,
) -> Result<(), String> {
    if state.provider == config.provider
        && state.model == request.model
        && state.endpoint == config.url
    {
        return Ok(());
    }
    Err("reasoning continuation belongs to another provider, model or endpoint; resume with its original profile, compact the completed incompatible history, or create a new strand".into())
}

fn incomplete() -> String {
    "incomplete reasoning continuation: keep the assistant exchange and all of its tool calls and results together; wait for active tools or compact completed exchanges".into()
}
