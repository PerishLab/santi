use ratatui::style::{Color, Modifier, Style};

use super::Tone;

pub(super) struct Painted {
    pub(super) text: String,
    pub(super) marks: Vec<(usize, usize, Style)>,
}

pub(super) struct Block {
    fence: bool,
}

impl Block {
    pub(super) fn new() -> Self {
        Self { fence: false }
    }

    pub(super) fn line(&mut self, raw: &str, tone: Tone) -> Painted {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            self.fence = !self.fence;
            let label = trimmed.trim_start_matches('`').trim();
            let text = if label.is_empty() {
                "───".to_string()
            } else {
                format!("── {label} ")
            };
            return whole(text, dim());
        }
        if self.fence {
            return whole(format!("│ {raw}"), code());
        }
        if let Some(rest) = heading(trimmed) {
            return whole(rest.to_string(), strong(tone));
        }
        if matches!(trimmed, "---" | "***" | "___") {
            return whole("─".repeat(32), dim());
        }
        if let Some(rest) = trimmed.strip_prefix("> ") {
            return whole(format!("│ {rest}"), dim());
        }
        let indent = " ".repeat(raw.len() - trimmed.len());
        let listed = ["- ", "* ", "+ "]
            .iter()
            .find_map(|mark| trimmed.strip_prefix(mark));
        match listed {
            Some(rest) => {
                let mut held = inline(rest);
                let lead = format!("{indent}• ");
                let shift = lead.len();
                for mark in &mut held.marks {
                    mark.0 += shift;
                    mark.1 += shift;
                }
                held.text = format!("{lead}{}", held.text);
                held
            }
            None => inline(raw),
        }
    }
}

fn whole(text: String, style: Style) -> Painted {
    let marks = vec![(0, text.len(), style)];
    Painted { text, marks }
}

fn heading(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches('#');
    let hashes = line.len() - rest.len();
    (1..=6).contains(&hashes).then(|| rest.strip_prefix(' '))?
}

fn inline(raw: &str) -> Painted {
    let mut text = String::with_capacity(raw.len());
    let mut marks = Vec::new();
    let mut rest = raw;
    while let Some(open) = rest.find(['*', '`']) {
        let fence = if rest[open..].starts_with("**") {
            "**"
        } else if rest[open..].starts_with('`') {
            "`"
        } else {
            text.push_str(&rest[..=open]);
            rest = &rest[open + 1..];
            continue;
        };
        let after = open + fence.len();
        let Some(shut) = rest[after..].find(fence) else {
            text.push_str(&rest[..after]);
            rest = &rest[after..];
            continue;
        };
        text.push_str(&rest[..open]);
        let start = text.len();
        text.push_str(&rest[after..after + shut]);
        let style = if fence == "`" { code() } else { bold() };
        marks.push((start, text.len(), style));
        rest = &rest[after + shut + fence.len()..];
    }
    text.push_str(rest);
    Painted { text, marks }
}

fn strong(tone: Tone) -> Style {
    match tone {
        Tone::You => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        Tone::Soul => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
        _ => bold(),
    }
}

fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

fn code() -> Style {
    Style::new().fg(Color::Magenta)
}

fn dim() -> Style {
    Style::new().add_modifier(Modifier::DIM)
}
