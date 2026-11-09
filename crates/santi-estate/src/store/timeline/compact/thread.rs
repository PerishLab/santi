use super::rule::Rule;
use super::scan::Scan;
use crate::store::read;
use keel::Tx;
use keel::adapt::{Error, db::Sqlite};
use santi_model::thinking;
use std::collections::BTreeMap;

type Ends = BTreeMap<i64, (i64, usize)>;

pub(super) struct Thread {
    pub(super) rules: Vec<Rule>,
}

pub(super) enum Recorded {
    Legacy,
    Broken,
    Round(Box<thinking::Round>),
}

pub(super) struct Frame {
    pub(super) at: i64,
    pub(super) turn: i64,
    pub(super) round: Recorded,
}

pub(super) struct Call {
    pub(super) at: i64,
    pub(super) key: i64,
    pub(super) tool: String,
    pub(super) turn: i64,
}

struct Build<'a> {
    key: &'a str,
    frames: &'a [Frame],
    calls: &'a [Call],
    assigned: &'a [Option<usize>],
    ends: &'a Ends,
}

pub(super) async fn load(tx: &mut Tx<'_, Sqlite>, strand: &str) -> Result<Thread, Error> {
    let key = read::need(tx, "Strand", "tag", strand).await?.to_string();
    let frames = Scan(tx).frames(&key).await?;
    let calls = Scan(tx).calls(&key).await?;
    let ends = Scan(tx).results(&key).await?;
    let mut assigned = vec![None; calls.len()];
    for (index, call) in calls.iter().enumerate() {
        if let Some(frame) = frames
            .iter()
            .rposition(|frame| frame.at < call.at && frame.turn == call.turn)
        {
            assigned[index] = Some(frame);
        }
    }
    let build = Build {
        key: &key,
        frames: &frames,
        calls: &calls,
        assigned: &assigned,
        ends: &ends,
    };
    let mut rules = Vec::new();
    for index in 0..frames.len() {
        build.add(tx, index, &mut rules).await?;
    }
    for (index, call) in calls.iter().enumerate() {
        if assigned[index].is_none() {
            rules.push(legacy(call, &ends));
        }
    }
    Ok(Thread { rules })
}

impl Build<'_> {
    async fn add(
        &self,
        tx: &mut Tx<'_, Sqlite>,
        index: usize,
        rules: &mut Vec<Rule>,
    ) -> Result<(), Error> {
        let frame = &self.frames[index];
        let held = self
            .calls
            .iter()
            .enumerate()
            .filter(|(slot, _)| self.assigned[*slot] == Some(index))
            .map(|(_, call)| call)
            .collect::<Vec<_>>();
        let associated = held
            .iter()
            .map(|call| {
                let end = single(self.ends, call.key).filter(|end| *end >= call.at);
                (call.at, end)
            })
            .collect::<Vec<_>>();
        let bound = extent(self.frames, index, &associated);
        match &frame.round {
            Recorded::Broken => refuse(rules, frame.at, bound, &associated),
            Recorded::Legacy => rules.extend(held.into_iter().map(|call| legacy(call, self.ends))),
            Recorded::Round(round) if !agree(round) => refuse(rules, frame.at, bound, &associated),
            Recorded::Round(round) if round.calls.is_empty() && !held.is_empty() => {
                refuse(rules, frame.at, bound, &associated)
            }
            Recorded::Round(round) if round.calls.is_empty() => {
                let pending = Pending {
                    at: frame.at,
                    next: self.frames.get(index + 1).map(|frame| frame.at),
                    text: round.text,
                };
                if let Some(rule) = plain(tx, self.key, pending).await? {
                    rules.push(rule);
                }
            }
            Recorded::Round(round) => {
                let Some(end) = complete(round, &held, self.ends) else {
                    refuse(rules, frame.at, bound, &associated);
                    return Ok(());
                };
                let text = Scan(tx).locate(self.key, frame.at, Some(end)).await?;
                if round.text > 0 && text.is_none() {
                    refuse(rules, frame.at, bound, &associated);
                } else {
                    rules.push(Rule::Exchange {
                        start: frame.at,
                        end: Some(end),
                    });
                }
            }
        }
        Ok(())
    }
}

fn extent(frames: &[Frame], index: usize, associated: &[(i64, Option<i64>)]) -> Option<i64> {
    let mut bound = frames.get(index + 1)?.at - 1;
    for &(at, end) in associated {
        bound = bound.max(at);
        if let Some(end) = end {
            bound = bound.max(end);
        }
    }
    Some(bound)
}

fn refuse(
    rules: &mut Vec<Rule>,
    start: i64,
    bound: Option<i64>,
    associated: &[(i64, Option<i64>)],
) {
    let Some(bound) = bound else {
        rules.push(Rule::Broken { start, end: None });
        return;
    };
    for &(at, end) in associated {
        match end {
            Some(end) => rules.push(Rule::Exchange {
                start: at,
                end: Some(end),
            }),
            None => rules.push(Rule::Broken {
                start: at,
                end: Some(bound),
            }),
        }
    }
    rules.push(Rule::Broken {
        start,
        end: Some(bound),
    });
}

struct Pending {
    at: i64,
    next: Option<i64>,
    text: usize,
}

async fn plain(
    tx: &mut Tx<'_, Sqlite>,
    strand: &str,
    pending: Pending,
) -> Result<Option<Rule>, Error> {
    let rule = match Scan(tx).locate(strand, pending.at, pending.next).await? {
        Some(text) => Rule::Anchor {
            at: pending.at,
            text: Some(text),
        },
        None if pending.text == 0 => return Ok(None),
        None => match pending.next {
            Some(next) => Rule::Broken {
                start: pending.at,
                end: Some(next - 1),
            },
            None => Rule::Anchor {
                at: pending.at,
                text: None,
            },
        },
    };
    Ok(Some(rule))
}

fn agree(round: &thinking::Round) -> bool {
    round
        .continuation
        .as_ref()
        .is_none_or(|continuation| continuation.calls == round.calls.len())
}

fn legacy(call: &Call, ends: &BTreeMap<i64, (i64, usize)>) -> Rule {
    Rule::Exchange {
        start: call.at,
        end: single(ends, call.key).filter(|end| *end >= call.at),
    }
}

fn single(ends: &BTreeMap<i64, (i64, usize)>, call: i64) -> Option<i64> {
    match ends.get(&call) {
        Some((end, 1)) => Some(*end),
        _ => None,
    }
}

fn complete(
    round: &thinking::Round,
    held: &[&Call],
    ends: &BTreeMap<i64, (i64, usize)>,
) -> Option<i64> {
    if round.calls.len() != held.len() {
        return None;
    }
    let mut used = vec![false; held.len()];
    let mut last: Option<i64> = None;
    for name in &round.calls {
        let mut found = None;
        for (index, call) in held.iter().enumerate() {
            if !used[index] && &call.tool == name {
                used[index] = true;
                found = Some(call);
                break;
            }
        }
        let call = found?;
        let end = single(ends, call.key)?;
        if end < call.at {
            return None;
        }
        last = Some(last.map_or(end, |held: i64| held.max(end)));
    }
    last
}
