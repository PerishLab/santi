use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::layout::{editor, text};
use super::state::{Entry, State};

const EDITOR: usize = 8;

pub(super) fn draw(frame: &mut Frame<'_>, state: &mut State) {
    let whole = frame.area();
    let room = (whole.height as usize)
        .saturating_sub(3)
        .div_ceil(3)
        .clamp(1, EDITOR);
    let editor = editor(
        &state.typed,
        state.cursor,
        whole.width.max(1) as usize,
        room,
    );
    let areas = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(editor.lines.len() as u16),
        Constraint::Length(1),
    ])
    .split(whole);
    let top = areas[0];
    let stream = areas[1];
    let input = areas[2];
    let footer = areas[3];
    let body = folded(state, stream.width.max(1) as usize);
    state.rendered(body, stream.y, stream.height.max(1) as usize);
    let shown = (state.scroll..state.scroll + stream.height as usize)
        .map_while(|row| state.line(row).map(|line| highlighted(line, state, row)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Span::styled(
            state.top(),
            Style::new().add_modifier(Modifier::BOLD),
        )),
        top,
    );
    frame.render_widget(Paragraph::new(shown), stream);
    frame.render_widget(
        Paragraph::new(
            editor
                .lines
                .iter()
                .map(|line| Line::from(line.as_str()))
                .collect::<Vec<_>>(),
        ),
        input,
    );
    frame.render_widget(
        Paragraph::new(Span::styled(
            state.footer(),
            Style::new().add_modifier(Modifier::DIM),
        )),
        footer,
    );
    let column = input.x.saturating_add(editor.column as u16);
    let row = input.y.saturating_add(editor.row as u16);
    frame.set_cursor_position((
        column.min(input.right().saturating_sub(1)),
        row.min(input.bottom().saturating_sub(1)),
    ));
}

fn highlighted<'a>(line: &'a str, state: &State, row: usize) -> Line<'a> {
    let Some((from, to)) = state.highlight(row) else {
        return Line::from(line);
    };
    let (start, end) = (cut(line, from), cut(line, to));
    let mark = Style::new().add_modifier(Modifier::REVERSED);
    Line::from(vec![
        Span::raw(&line[..start]),
        Span::styled(&line[start..end], mark),
        Span::raw(&line[end..]),
    ])
}

fn cut(line: &str, column: usize) -> usize {
    let mut used = 0;
    for (offset, grapheme) in line.grapheme_indices(true) {
        if used >= column {
            return offset;
        }
        used += grapheme.width();
    }
    line.len()
}

fn folded(state: &State, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    if state.omitted > 0 {
        out.push(format!(
            "history: {} earlier messages omitted",
            state.omitted
        ));
    }
    for entry in &state.entries {
        out.extend(shown(entry, state.verbose, width));
    }
    out
}

fn shown(entry: &Entry, verbose: bool, width: usize) -> Vec<String> {
    match entry {
        Entry::Said(lines) => lines.iter().flat_map(|line| text(line, width)).collect(),
        Entry::Activity { items } if verbose => items
            .iter()
            .flat_map(|(_, line)| text(&format!("  {line}"), width))
            .collect(),
        Entry::Activity { items } => text(&Entry::summary(items), width),
        Entry::Notice(line) => text(line, width),
    }
}
