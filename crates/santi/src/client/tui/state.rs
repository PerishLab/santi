use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[path = "state/edit.rs"]
mod edit;

#[path = "state/glyph.rs"]
mod glyph;

#[path = "state/stroke.rs"]
mod stroke;

#[path = "state/view.rs"]
mod view;

#[path = "state/recover.rs"]
pub(super) mod recover;

#[cfg(test)]
#[path = "state/tests.rs"]
mod tests;

use super::Kind;
use super::parse::{Spoken, now};

const DRAFT: usize = 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Step {
    Stay,
    Leave,
    Reload,
    Refresh,
    Speak(String),
    Copy(String),
    Stop(String),
    Name(String, String),
    Switch(String),
    Listing(String),
    Jobs(String),
    Idle,
}

pub(super) enum Beat {
    Open(String),
    Speech(String),
    Event(Kind, Option<String>, String),
    Lost(String),
    Live,
    Settled(String),
    Broken(String),
    Unsettled(String),
}

pub(super) enum Entry {
    Said(Spoken),
    Activity {
        turn: Option<String>,
        items: Vec<(Kind, String)>,
    },
    Elided(i64),
    Notice(String),
}

pub(super) fn column(line: &str, column: usize) -> usize {
    let mut used = 0;
    for (offset, grapheme) in line.grapheme_indices(true) {
        if used >= column {
            return offset;
        }
        used += UnicodeWidthStr::width(grapheme);
    }
    line.len()
}

pub(super) struct State {
    pub(super) soul: String,
    pub(super) strand: String,
    pub(super) entries: Vec<Entry>,
    pub(super) verbose: bool,
    pub(super) typed: String,
    pub(super) cursor: usize,
    preferred: Option<usize>,
    pub(super) context: String,
    pub(super) scroll: usize,
    follow: bool,
    unseen: bool,
    viewport: usize,
    total: usize,
    body: Vec<String>,
    origin: u16,
    selection: Option<view::Span>,
    pub(super) turn: Option<String>,
    pub(super) since: Option<std::time::Instant>,
    pub(super) beats: usize,
    pub(super) busy: bool,
    pub(super) unsettled: bool,
    pub(super) deaf: bool,
    pub(super) omitted: usize,
    pub(super) revision: usize,
    pub(super) names: std::collections::HashMap<String, String>,
}

impl State {
    pub(super) fn new(
        soul: String,
        strand: String,
        names: std::collections::HashMap<String, String>,
    ) -> Self {
        Self {
            soul,
            strand,
            entries: Vec::new(),
            verbose: false,
            typed: String::new(),
            cursor: 0,
            preferred: None,
            context: String::from("context: unread"),
            scroll: 0,
            follow: true,
            unseen: false,
            viewport: 1,
            total: 0,
            body: Vec::new(),
            origin: 0,
            selection: None,
            turn: None,
            since: None,
            beats: 0,
            busy: false,
            unsettled: false,
            deaf: false,
            omitted: 0,
            revision: 0,
            names,
        }
    }

    pub(super) fn seed(&mut self, omitted: usize, header: Vec<String>, spoken: Vec<Spoken>) {
        self.omitted = omitted;
        self.entries = header.into_iter().map(Entry::Notice).collect();
        let mut last = 0;
        for held in spoken {
            let gap = held.seq.saturating_sub(last).saturating_sub(1);
            if last > 0 && gap > 0 {
                self.entries.push(Entry::Elided(gap));
            }
            last = held.seq;
            self.entries.push(Entry::Said(held));
        }
        self.follow = true;
        self.changed();
    }

    pub(super) fn transcript(&self) -> String {
        let mut out = Vec::new();
        for entry in &self.entries {
            match entry {
                Entry::Said(held) => out.extend(held.lines.iter().cloned()),
                Entry::Activity { items, .. } => {
                    out.extend(items.iter().map(|(_, line)| line.clone()));
                }
                Entry::Elided(gap) => out.push(format!("{gap} entries not retained")),
                Entry::Notice(line) => out.push(line.clone()),
            }
        }
        out.join("\n")
    }

    pub(super) fn absorb(&mut self, beat: Beat) {
        match beat {
            Beat::Open(who) => {
                self.entries.push(Entry::Said(Spoken {
                    seq: 0,
                    stamp: now(),
                    who,
                    lines: vec![String::new()],
                }));
                self.changed();
            }
            Beat::Speech(text) => self.append(&text),
            Beat::Event(kind, turn, line) => {
                if let Some(held) = turn.clone() {
                    self.turn = Some(held);
                }
                self.activity(kind, turn, line);
            }
            Beat::Lost(detail) => {
                self.deaf = true;
                self.push(format!("stream lost: {detail}; reconnecting"));
            }
            Beat::Live => {
                if self.deaf {
                    self.deaf = false;
                    self.push(
                        "stream resumed; events during the outage are not recoverable from the \
                         server and are not replayed"
                            .to_string(),
                    );
                }
            }
            Beat::Settled(receipt) => {
                self.busy = false;
                self.unsettled = false;
                self.since = None;
                self.push(format!(
                    "send completed: receipt {receipt} is durably completed; do not resend"
                ));
            }
            Beat::Broken(detail) => {
                self.busy = false;
                self.unsettled = false;
                self.since = None;
                self.push(detail);
            }
            Beat::Unsettled(detail) => {
                self.busy = true;
                self.unsettled = true;
                self.push(detail);
            }
        }
    }

    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        if !self.follow {
            self.unseen = true;
        }
    }

    fn append(&mut self, text: &str) {
        if !matches!(self.entries.last(), Some(Entry::Said(_))) {
            self.entries.push(Entry::Said(Spoken {
                seq: 0,
                stamp: now(),
                who: "soul".to_string(),
                lines: vec![String::new()],
            }));
        }
        let Some(Entry::Said(held)) = self.entries.last_mut() else {
            return;
        };
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 {
                held.lines.push(String::new());
            }
            match held.lines.last_mut() {
                Some(line) => line.push_str(part),
                None => held.lines.push(part.to_string()),
            }
        }
        self.changed();
    }

    fn activity(&mut self, kind: Kind, turn: Option<String>, line: String) {
        if kind == Kind::Fault {
            self.push(line);
            return;
        }
        let same = match self.entries.last() {
            Some(Entry::Activity { turn: held, .. }) => *held == turn || turn.is_none(),
            _ => false,
        };
        if !same {
            self.entries.push(Entry::Activity {
                turn,
                items: Vec::new(),
            });
        }
        let Some(Entry::Activity { items, .. }) = self.entries.last_mut() else {
            return;
        };
        items.push((kind, line));
        self.changed();
    }

    pub(super) fn push(&mut self, line: String) {
        self.entries.push(Entry::Notice(line));
        self.changed();
    }
}
