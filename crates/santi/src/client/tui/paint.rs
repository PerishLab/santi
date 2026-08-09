use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use super::layout::{editor, text};
use super::parse::Spoken;
use super::state::column;
use super::state::{Entry, State};

const EDITOR: usize = 8;
const RAIL: &str = "▏";
const MARGIN: &str = "  ";

#[path = "paint/tone.rs"]
mod tone;

pub(super) use tone::Tone;

#[path = "paint/clip.rs"]
pub(super) mod clip;

#[path = "paint/markup.rs"]
mod markup;

use markup::{Block, Painted};

pub(super) struct Cache {
    width: usize,
    revision: usize,
    shards: Vec<Shard>,
}

impl Cache {
    pub(super) fn new() -> Self {
        Self {
            width: 0,
            revision: usize::MAX,
            shards: Vec::new(),
        }
    }

    fn shards(&mut self, state: &State, width: usize) -> &[Shard] {
        if self.width != width || self.revision != state.revision {
            self.shards = folded(state, width);
            self.width = width;
            self.revision = state.revision;
        }
        &self.shards
    }
}

pub(super) fn draw(frame: &mut Frame<'_>, state: &mut State, cache: &mut Cache) {
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
        Constraint::Length(1),
        Constraint::Length(editor.lines.len() as u16),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(whole);
    let top = areas[0];
    let stream = areas[1];
    let rule = areas[2];
    let input = areas[3];
    let under = areas[4];
    let footer = areas[5];
    let width = stream.width.max(1) as usize;
    let fresh = cache.width != width || cache.revision != state.revision;
    cache.shards(state, width);
    if fresh {
        let body = cache.shards.iter().map(|held| held.text.clone()).collect();
        state.rendered(body, stream.y, stream.height.max(1) as usize);
    } else {
        state.reframe(stream.y, stream.height.max(1) as usize);
    }
    let shown = (state.scroll..state.scroll + stream.height as usize)
        .map_while(|row| {
            let line = state.line(row)?;
            let held = cache.shards.get(row)?;
            Some(highlighted(line, state, row, held))
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(Span::styled(
            state.top(),
            Style::new().add_modifier(Modifier::BOLD),
        )),
        top,
    );
    frame.render_widget(Paragraph::new(shown), stream);
    for edge in [rule, under] {
        frame.render_widget(
            Paragraph::new(Span::styled(
                "─".repeat(edge.width as usize),
                Style::new().add_modifier(Modifier::DIM),
            )),
            edge,
        );
    }
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

fn highlighted<'a>(line: &'a str, state: &State, row: usize, held: &Shard) -> Line<'a> {
    let plain = held.tone.style();
    let chosen = state
        .highlight(row)
        .map(|(from, to)| (column(line, from), column(line, to)));
    let mut out = Vec::new();
    let mut at = 0;
    for (start, end, style) in &held.marks {
        let (start, end) = ((*start).min(line.len()), (*end).min(line.len()));
        if start < at || start >= end {
            continue;
        }
        out.push(Span::styled(&line[at..start], plain));
        out.push(Span::styled(&line[start..end], plain.patch(*style)));
        at = end;
    }
    out.push(Span::styled(&line[at..], plain));
    let Some((start, end)) = chosen else {
        return Line::from(out);
    };
    Line::from(reversed(line, start, end, plain))
}

fn reversed(line: &str, start: usize, end: usize, plain: Style) -> Vec<Span<'_>> {
    let mark = plain.add_modifier(Modifier::REVERSED);
    vec![
        Span::styled(&line[..start], plain),
        Span::styled(&line[start..end], mark),
        Span::styled(&line[end..], plain),
    ]
}

fn inset(mut shard: Shard) -> Shard {
    let shift = MARGIN.len();
    for mark in &mut shard.marks {
        mark.0 += shift;
        mark.1 += shift;
    }
    shard.text = format!("{MARGIN}{}", shard.text);
    shard
}

fn folded(state: &State, width: usize) -> Vec<Shard> {
    let mut out = Vec::new();
    if state.omitted > 0 {
        out.push(Shard {
            tone: Tone::Notice,
            text: format!("{} earlier messages omitted", state.omitted),
            marks: Vec::new(),
        });
    }
    for entry in &state.entries {
        out.extend(shown(entry, state.verbose, width).into_iter().map(inset));
    }
    out
}

fn shown(entry: &Entry, verbose: bool, width: usize) -> Vec<Shard> {
    let body = width.saturating_sub(MARGIN.len()).max(1);
    match entry {
        Entry::Said(said) => spoken(said, body),
        Entry::Activity { items, .. } if verbose => items
            .iter()
            .flat_map(|(_, line)| text(&format!("  {line}"), width))
            .map(|held| Shard {
                tone: Tone::Activity,
                text: held,
                marks: Vec::new(),
            })
            .collect(),
        Entry::Activity { turn, items } => quiet(width, &Entry::summary(turn.as_deref(), items)),
        Entry::Elided(gap) => quiet(width, &format!("⋮ {gap} thinking and tool entries")),
        Entry::Notice(line) => text(line, width)
            .into_iter()
            .map(|held| Shard {
                tone: Tone::Notice,
                text: held,
                marks: Vec::new(),
            })
            .collect(),
    }
}

fn spoken(said: &Spoken, width: usize) -> Vec<Shard> {
    let tone = speaker(&said.who);
    let coordinate = format!("{RAIL}{}", said.stamp);
    let mut out = vec![Shard {
        tone: Tone::Activity,
        marks: vec![(0, coordinate.len(), Style::new().fg(hue(&said.who)))],
        text: coordinate,
    }];
    let mut block = Block::new();
    for line in &said.lines {
        let Painted { text: held, marks } = block.line(line, tone);
        let mut wrapped = text(&held, width).into_iter();
        let Some(first) = wrapped.next() else {
            continue;
        };
        out.push(Shard {
            tone: Tone::Body,
            text: first,
            marks,
        });
        for rest in wrapped {
            out.push(Shard {
                tone: Tone::Body,
                text: rest,
                marks: Vec::new(),
            });
        }
    }
    out.push(Shard {
        tone: Tone::Body,
        text: String::new(),
        marks: Vec::new(),
    });
    out
}

fn quiet(width: usize, line: &str) -> Vec<Shard> {
    text(line, width)
        .into_iter()
        .map(|held| Shard {
            tone: Tone::Activity,
            text: held,
            marks: Vec::new(),
        })
        .collect()
}

pub(super) struct Shard {
    tone: Tone,
    text: String,
    marks: Vec<(usize, usize, Style)>,
}

fn hue(who: &str) -> Color {
    if who == "you" {
        Color::Cyan
    } else {
        Color::Green
    }
}

fn speaker(who: &str) -> Tone {
    if who == "you" { Tone::You } else { Tone::Soul }
}
