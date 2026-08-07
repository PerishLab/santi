use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::state::State;

pub(super) fn draw(frame: &mut Frame<'_>, state: &State) {
    let areas = Layout::vertical([
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(frame.area());
    let stream = areas[0];
    let body = folded(state, stream.width.max(1) as usize);
    let height = stream.height.max(1) as usize;
    let floor = body.len().saturating_sub(height);
    let top = floor.saturating_sub(state.scroll);
    let shown = body
        .into_iter()
        .skip(top)
        .take(height)
        .map(Line::from)
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(shown), stream);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("you> ", Style::new().add_modifier(Modifier::BOLD)),
            Span::raw(state.typed.as_str()),
        ])),
        areas[1],
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            state.status(),
            Style::new().add_modifier(Modifier::DIM),
        )),
        areas[2],
    );
    let column = areas[1].x + 5 + state.typed.width() as u16;
    frame.set_cursor_position((column.min(areas[1].right().saturating_sub(1)), areas[1].y));
}

fn folded(state: &State, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    if state.omitted > 0 {
        out.push(format!(
            "history: {} earlier messages omitted",
            state.omitted
        ));
    }
    for line in &state.spoken {
        out.extend(wrapped(line, width));
    }
    out
}

fn wrapped(line: &str, width: usize) -> Vec<String> {
    if line.width() <= width {
        return vec![line.to_string()];
    }
    let mut out = Vec::new();
    let mut held = String::new();
    let mut span = 0;
    for character in line.chars() {
        let cost = character.width().unwrap_or(0);
        if span + cost > width && !held.is_empty() {
            out.push(std::mem::take(&mut held));
            span = 0;
        }
        held.push(character);
        span += cost;
    }
    if !held.is_empty() {
        out.push(held);
    }
    out
}
