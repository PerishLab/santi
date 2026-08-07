use super::keys::Stroke;

pub(super) enum Step {
    Stay,
    Leave,
    Refresh,
    Speak(String),
    Idle,
}

pub(super) enum Beat {
    Text(String),
    Settled(String),
    Broken(String),
}

pub(super) struct State {
    pub(super) soul: String,
    pub(super) strand: String,
    pub(super) spoken: Vec<String>,
    pub(super) typed: String,
    pub(super) context: String,
    pub(super) scroll: usize,
    pub(super) busy: bool,
    pub(super) omitted: usize,
}

impl State {
    pub(super) fn new(soul: String, strand: String) -> Self {
        Self {
            soul,
            strand,
            spoken: Vec::new(),
            typed: String::new(),
            context: String::from("context: unread"),
            scroll: 0,
            busy: false,
            omitted: 0,
        }
    }

    pub(super) fn seed(&mut self, omitted: usize, spoken: Vec<String>) {
        self.omitted = omitted;
        self.spoken = spoken;
    }

    pub(super) fn absorb(&mut self, beat: Beat) {
        match beat {
            Beat::Text(text) => self.append(&text),
            Beat::Settled(receipt) => {
                self.busy = false;
                self.push(format!(
                    "send completed: receipt {receipt} is durably completed; do not resend"
                ));
            }
            Beat::Broken(detail) => {
                self.busy = false;
                self.push(detail);
            }
        }
        self.scroll = 0;
    }

    fn append(&mut self, text: &str) {
        for (index, part) in text.split('\n').enumerate() {
            if index > 0 {
                self.spoken.push(String::new());
            }
            match self.spoken.last_mut() {
                Some(line) => line.push_str(part),
                None => self.spoken.push(part.to_string()),
            }
        }
    }

    pub(super) fn push(&mut self, line: String) {
        match self.spoken.last_mut() {
            Some(held) if held.is_empty() => *held = line,
            _ => self.spoken.push(line),
        }
        self.spoken.push(String::new());
        self.scroll = 0;
    }

    pub(super) fn key(&mut self, character: char) {
        self.typed.push(character);
    }

    pub(super) fn erase(&mut self) {
        self.typed.pop();
    }

    pub(super) fn taken(&mut self) -> Option<String> {
        let text = self.typed.trim().to_string();
        if text.is_empty() {
            return None;
        }
        self.typed.clear();
        Some(text)
    }

    pub(super) fn up(&mut self, span: usize) {
        let ceiling = self.spoken.len().saturating_sub(1);
        self.scroll = (self.scroll + span).min(ceiling);
    }

    pub(super) fn down(&mut self, span: usize) {
        self.scroll = self.scroll.saturating_sub(span);
    }

    pub(super) fn status(&self) -> String {
        let state = if self.busy { "sending" } else { "ready" };
        let seen = if self.scroll == 0 {
            String::new()
        } else {
            format!("  scrolled {}", self.scroll)
        };
        format!(
            "{} · {} · {} · {state}{seen}",
            short(&self.soul),
            short(&self.strand),
            self.context
        )
    }
}

pub(super) fn short(id: &str) -> String {
    match id.split_once('_') {
        Some((kind, rest)) if rest.len() > 8 => format!("{kind}_{}", &rest[..8]),
        _ => id.to_string(),
    }
}

impl State {
    pub(super) fn struck(&mut self, stroke: Option<Stroke>) -> Step {
        let Some(stroke) = stroke else {
            return Step::Leave;
        };
        match stroke {
            Stroke::Quit => Step::Leave,
            Stroke::Enter => self.entered(),
            Stroke::Erase => {
                self.erase();
                Step::Stay
            }
            Stroke::Typed(character) => {
                self.key(character);
                Step::Stay
            }
            Stroke::Up => {
                self.up(5);
                Step::Stay
            }
            Stroke::Down => {
                self.down(5);
                Step::Stay
            }
            Stroke::Redraw => Step::Stay,
        }
    }

    fn entered(&mut self) -> Step {
        let Some(text) = self.taken() else {
            return Step::Stay;
        };
        match text.as_str() {
            "/exit" => Step::Leave,
            "/status" => Step::Refresh,
            _ => self.spoken(text),
        }
    }

    fn spoken(&mut self, text: String) -> Step {
        self.push(format!("you> {text}"));
        if self.busy {
            self.push(
                "a send is already in flight; wait for its receipt before sending again"
                    .to_string(),
            );
            return Step::Stay;
        }
        self.busy = true;
        Step::Speak(text)
    }

    pub(super) fn heard(&mut self, beat: Option<Beat>) -> Step {
        let Some(beat) = beat else {
            return Step::Stay;
        };
        let settled = matches!(beat, Beat::Settled(_) | Beat::Broken(_));
        self.absorb(beat);
        if settled { Step::Refresh } else { Step::Stay }
    }
}
