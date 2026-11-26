use std::collections::HashMap;

use super::{Emit, Kind, Presentation, Shown, field, owner, render};
use crate::cli::Format;

impl Kind {
    fn of(event: &str, beat: &str, data: &str) -> Self {
        match (event, beat) {
            ("thinking", _) => Self::Thinking,
            ("tool", _) => Self::Tool,
            ("turn", "failed")
                if field(data, &["payload", "error", "code"]).as_deref()
                    == Some("runtime.turn.interrupted")
                    && field(data, &["payload", "error", "context", "cause"]).as_deref()
                        == Some("operator") =>
            {
                Self::Interrupted
            }
            ("turn", "failed") | ("transition", _) => Self::Fault,
            ("turn", _) => Self::Turn,
            _ => Self::Other,
        }
    }
}

pub(super) struct Display {
    presentation: Presentation,
    speaking: bool,
    newline: bool,
    spoken: HashMap<String, String>,
    gap: bool,
    activity: Option<(String, String)>,
}

impl Display {
    pub(super) fn new(presentation: Presentation) -> Self {
        Self {
            presentation,
            speaking: false,
            newline: false,
            spoken: HashMap::new(),
            gap: false,
            activity: None,
        }
    }
    pub(super) fn write(&mut self, output: &mut impl Emit, event: &str, data: &str) {
        if event == "gap" {
            self.gap = true;
            self.finish(output);
        }
        match self.presentation {
            Presentation::Watch(Format::Raw) => {
                if event != "open" {
                    let beat = field(data, &["payload", "beat"]).unwrap_or_default();
                    let turn = owner(data);
                    output.event(Shown {
                        kind: Kind::of(event, &beat, data),
                        turn: turn.as_deref(),
                        line: data,
                    });
                }
            }
            Presentation::Watch(Format::Filtered) => self.filtered(output, event, data),
            Presentation::Tui => self.tui(output, event, data),
        }
    }
    fn filtered(&mut self, output: &mut impl Emit, event: &str, data: &str) {
        let beat = field(data, &["payload", "beat"]);
        if event == "gap"
            || event == "turn"
                && matches!(beat.as_deref(), Some("started" | "completed" | "failed"))
        {
            self.activity = None;
        }
        if event == "turn" && beat.as_deref() == Some("active") {
            let activity = field(data, &["payload", "activity", "turn"])
                .zip(field(data, &["payload", "activity", "state"]))
                .filter(|(turn, state)| {
                    !turn.is_empty()
                        && matches!(
                            state.as_str(),
                            "requesting" | "thinking" | "generating" | "calling" | "running"
                        )
                });
            let duplicate = activity.is_some() && self.activity == activity;
            self.activity = activity;
            if duplicate {
                return;
            }
        }
        line(output, event, data);
    }
    fn tui(&mut self, output: &mut impl Emit, event: &str, data: &str) {
        let beat = field(data, &["payload", "beat"]);
        if event == "message" && beat.as_deref() == Some("delta") {
            let Some(text) = field(data, &["payload", "text"]) else {
                return;
            };
            let text = speech(&text);
            if text.is_empty() {
                return;
            }
            if !self.speaking {
                output.open("soul");
                self.speaking = true;
            }
            output.speech(&text);
            self.newline = text.ends_with('\n');
            if let Some(turn) = field(data, &["payload", "turn"]) {
                self.spoken.entry(turn).or_default().push_str(&text);
            }
            return;
        }
        self.finish(output);
        let completed = event == "message" && beat.as_deref() == Some("completed");
        let duplicate = completed
            && !self.gap
            && field(data, &["payload", "turn"]).is_some_and(|turn| {
                field(data, &["payload", "message", "text"])
                    .map(|text| speech(&text))
                    .is_some_and(|text| self.spoken.get(&turn) == Some(&text))
            });
        if !duplicate {
            line(output, event, data);
        }
        if event == "turn"
            && matches!(beat.as_deref(), Some("completed") | Some("failed"))
            && let Some(turn) = field(data, &["payload", "turn"])
        {
            self.spoken.remove(&turn);
        }
    }
    pub(super) fn finish(&mut self, output: &mut impl Emit) {
        if self.speaking && !self.newline {
            output.speech("\n");
        }
        self.speaking = false;
        self.newline = false;
    }
}

fn line(output: &mut impl Emit, event: &str, data: &str) {
    if let Some(line) = render(event, data) {
        let beat = field(data, &["payload", "beat"]).unwrap_or_default();
        let turn = owner(data);
        output.event(Shown {
            kind: Kind::of(event, &beat, data),
            turn: turn.as_deref(),
            line: &line,
        });
    }
}

fn speech(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}

#[cfg(test)]
mod check;
