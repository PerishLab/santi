use std::collections::HashMap;

use super::{Emit, Kind, Presentation, Shown, json_field, render_watch_event};
use crate::cli::WatchFormat;

pub(super) struct Display {
    presentation: Presentation,
    speaking: bool,
    newline: bool,
    spoken: HashMap<String, String>,
    gap: bool,
}

impl Display {
    pub(super) fn new(presentation: Presentation) -> Self {
        Self {
            presentation,
            speaking: false,
            newline: false,
            spoken: HashMap::new(),
            gap: false,
        }
    }
    pub(super) fn write(&mut self, output: &mut impl Emit, event: &str, data: &str) {
        if event == "gap" {
            self.gap = true;
            self.finish(output);
        }
        match self.presentation {
            Presentation::Watch(WatchFormat::Raw) => {
                if event != "open" {
                    let beat = json_field(data, &["payload", "beat"]).unwrap_or_default();
                    let turn = json_field(data, &["payload", "turn"]);
                    output.event(Shown {
                        kind: Kind::of(event, &beat),
                        turn: turn.as_deref(),
                        line: data,
                    });
                }
            }
            Presentation::Watch(WatchFormat::Filtered) => line(output, event, data),
            Presentation::Tui => self.tui(output, event, data),
        }
    }
    fn tui(&mut self, output: &mut impl Emit, event: &str, data: &str) {
        let beat = json_field(data, &["payload", "beat"]);
        if event == "message" && beat.as_deref() == Some("delta") {
            let Some(text) = json_field(data, &["payload", "text"]) else {
                return;
            };
            let text = speech(&text);
            if text.is_empty() {
                return;
            }
            if !self.speaking {
                output.speech("soul> ");
                self.speaking = true;
            }
            output.speech(&text);
            self.newline = text.ends_with('\n');
            if let Some(turn) = json_field(data, &["payload", "turn"]) {
                self.spoken.entry(turn).or_default().push_str(&text);
            }
            return;
        }
        self.finish(output);
        let completed = event == "message" && beat.as_deref() == Some("completed");
        let duplicate = completed
            && !self.gap
            && json_field(data, &["payload", "turn"]).is_some_and(|turn| {
                json_field(data, &["payload", "message", "text"])
                    .map(|text| speech(&text))
                    .is_some_and(|text| self.spoken.get(&turn) == Some(&text))
            });
        if !duplicate {
            line(output, event, data);
        }
        if event == "turn"
            && matches!(beat.as_deref(), Some("completed") | Some("failed"))
            && let Some(turn) = json_field(data, &["payload", "turn"])
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
    if let Some(line) = render_watch_event(event, data) {
        let beat = json_field(data, &["payload", "beat"]).unwrap_or_default();
        let turn = json_field(data, &["payload", "turn"]);
        output.event(Shown {
            kind: Kind::of(event, &beat),
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
