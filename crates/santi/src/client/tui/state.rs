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

use super::Kind;

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
    Idle,
}

pub(super) enum Beat {
    Speech(String),
    Event(Kind, Option<String>, String),
    Lost(String),
    Live,
    Settled(String),
    Broken(String),
    Unsettled(String),
}

pub(super) enum Entry {
    Said(Vec<String>),
    Activity { items: Vec<(Kind, String)> },
    Notice(String),
}

impl Entry {
    pub(super) fn summary(items: &[(Kind, String)]) -> String {
        let count = |wanted: Kind| items.iter().filter(|(kind, _)| *kind == wanted).count();
        let mut parts = Vec::new();
        for (kind, label) in [
            (Kind::Thinking, "thinking"),
            (Kind::Tool, "tool"),
            (Kind::Turn, "turn"),
            (Kind::Other, "event"),
        ] {
            match count(kind) {
                0 => {}
                n => parts.push(format!("{label} {n}")),
            }
        }
        if parts.is_empty() {
            parts.push(format!("event {}", items.len()));
        }
        format!("▸ {}", parts.join(" · "))
    }
}

#[derive(Clone, Copy)]
pub(super) struct Span {
    anchor: (usize, usize),
    head: (usize, usize),
}

impl Span {
    fn ordered(self) -> ((usize, usize), (usize, usize)) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}

fn offset(line: &str, column: usize) -> usize {
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
    selection: Option<Span>,
    pub(super) turn: Option<String>,
    pub(super) busy: bool,
    pub(super) unsettled: bool,
    pub(super) deaf: bool,
    pub(super) omitted: usize,
}

impl State {
    pub(super) fn new(soul: String, strand: String) -> Self {
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
            busy: false,
            unsettled: false,
            deaf: false,
            omitted: 0,
        }
    }

    pub(super) fn seed(&mut self, omitted: usize, spoken: Vec<String>) {
        self.omitted = omitted;
        self.entries = if spoken.is_empty() {
            Vec::new()
        } else {
            vec![Entry::Said(spoken)]
        };
        self.follow = true;
    }

    pub(super) fn transcript(&self) -> String {
        let mut out = Vec::new();
        for entry in &self.entries {
            match entry {
                Entry::Said(lines) => out.extend(lines.iter().cloned()),
                Entry::Activity { items } => {
                    out.extend(items.iter().map(|(_, line)| line.clone()));
                }
                Entry::Notice(line) => out.push(line.clone()),
            }
        }
        out.join("\n")
    }

    pub(super) fn absorb(&mut self, beat: Beat) {
        match beat {
            Beat::Speech(text) => self.append(&text),
            Beat::Event(kind, turn, line) => {
                if let Some(turn) = turn {
                    self.turn = Some(turn);
                }
                self.activity(kind, line);
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
                self.push(format!(
                    "send completed: receipt {receipt} is durably completed; do not resend"
                ));
            }
            Beat::Broken(detail) => {
                self.busy = false;
                self.unsettled = false;
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
        if !self.follow {
            self.unseen = true;
        }
    }

    fn append(&mut self, text: &str) {
        if !matches!(self.entries.last(), Some(Entry::Said(_))) {
            self.entries.push(Entry::Said(vec![String::new()]));
        }
        let Some(Entry::Said(lines)) = self.entries.last_mut() else {
            return;
        };
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 {
                lines.push(String::new());
            }
            match lines.last_mut() {
                Some(line) => line.push_str(part),
                None => lines.push(part.to_string()),
            }
        }
        self.changed();
    }

    fn activity(&mut self, kind: Kind, line: String) {
        if kind == Kind::Fault {
            self.push(line);
            return;
        }
        if !matches!(self.entries.last(), Some(Entry::Activity { .. })) {
            self.entries.push(Entry::Activity { items: Vec::new() });
        }
        let Some(Entry::Activity { items }) = self.entries.last_mut() else {
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
