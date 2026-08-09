use unicode_width::UnicodeWidthStr;

use super::{Entry, Kind, State, Step, column};

fn brief(turn: &str) -> String {
    let rest = turn.strip_prefix("turn_").unwrap_or(turn);
    let held = rest.get(..8).unwrap_or(rest);
    format!("turn {held}  ")
}

impl Entry {
    pub(crate) fn summary(turn: Option<&str>, items: &[(Kind, String)]) -> String {
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
        let named = turn.map(brief).unwrap_or_default();
        format!("▸ {named}{}", parts.join("  "))
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Span {
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

impl State {
    pub(crate) fn rendered(&mut self, body: Vec<String>, origin: u16, visible_lines: usize) {
        self.body = body;
        self.origin = origin;
        self.viewport(self.body.len(), visible_lines);
    }

    pub(super) fn point(&self, x: u16, y: u16) -> (usize, usize) {
        let row = self.scroll + usize::from(y.saturating_sub(self.origin));
        (row.min(self.body.len().saturating_sub(1)), usize::from(x))
    }

    pub(super) fn press(&mut self, x: u16, y: u16) {
        let at = self.point(x, y);
        self.selection = Some(Span {
            anchor: at,
            head: at,
        });
    }

    pub(super) fn drag(&mut self, x: u16, y: u16) {
        let at = self.point(x, y);
        if let Some(span) = self.selection.as_mut() {
            span.head = at;
        }
    }

    pub(crate) fn line(&self, row: usize) -> Option<&str> {
        self.body.get(row).map(String::as_str)
    }

    pub(crate) fn highlight(&self, row: usize) -> Option<(usize, usize)> {
        let (start, end) = self.selection?.ordered();
        if start == end || row < start.0 || row > end.0 {
            return None;
        }
        let width = self.body.get(row).map_or(0, |line| line.width());
        let from = if row == start.0 { start.1 } else { 0 };
        let to = if row == end.0 { end.1 } else { width };
        (from < to).then_some((from, to.min(width)))
    }

    pub(super) fn selected(&self) -> Option<String> {
        let (start, end) = self.selection?.ordered();
        if start == end {
            return None;
        }
        let mut out = Vec::new();
        for row in start.0..=end.0 {
            let line = self.body.get(row)?;
            let from = if row == start.0 {
                column(line, start.1)
            } else {
                0
            };
            let to = if row == end.0 {
                column(line, end.1)
            } else {
                line.len()
            };
            out.push(line[from.min(to)..to].to_string());
        }
        Some(out.join("\n")).filter(|text| !text.is_empty())
    }

    pub(crate) fn viewport(&mut self, total_lines: usize, visible_lines: usize) {
        self.viewport = visible_lines.max(1);
        self.total = total_lines;
        let ceiling = total_lines.saturating_sub(self.viewport);
        if self.follow {
            self.scroll = ceiling;
            self.unseen = false;
        } else {
            self.scroll = self.scroll.min(ceiling);
            if self.scroll == ceiling {
                self.follow = true;
                self.unseen = false;
            }
        }
    }

    pub(super) fn higher(&mut self, span: usize) {
        self.follow = false;
        self.scroll = self.scroll.saturating_sub(span.max(1));
    }

    pub(super) fn lower(&mut self, span: usize) {
        self.scroll = self.scroll.saturating_add(span.max(1));
    }

    pub(super) fn bottom(&mut self) {
        self.follow = true;
        self.unseen = false;
    }

    pub(crate) fn reframe(&mut self, origin: u16, visible: usize) {
        self.origin = origin;
        let total = self.body.len();
        self.viewport(total, visible);
    }

    pub(crate) fn travel(&mut self, want: String) -> Step {
        if want.is_empty() {
            return Step::Listing("strands".to_string());
        }
        let found = self
            .names
            .iter()
            .find(|(_, called)| called.as_str() == want)
            .map(|(id, _)| id.clone());
        let next = found.unwrap_or(want);
        if next == self.strand {
            self.push(format!("already on {}", self.calling(&next)));
            return Step::Stay;
        }
        Step::Switch(next)
    }

    pub(crate) fn rename(&mut self, name: String) -> Step {
        let strand = self.strand.clone();
        if name.is_empty() {
            self.names.remove(&strand);
        } else {
            self.names.insert(strand.clone(), name.clone());
        }
        Step::Name(strand, name)
    }

    pub(crate) fn calling(&self, id: &str) -> String {
        match self.names.get(id) {
            Some(name) => name.clone(),
            None => short(id),
        }
    }

    pub(crate) fn tick(&mut self) {
        self.beats = self.beats.wrapping_add(1);
    }

    pub(crate) fn alive(&self) -> Option<String> {
        let since = self.since?;
        let spin = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        let mark = spin[self.beats % spin.len()];
        Some(format!("{mark} {}s", since.elapsed().as_secs()))
    }

    pub(crate) fn top(&self) -> String {
        let work = if self.unsettled {
            "receipt unsettled"
        } else if self.busy {
            "sending"
        } else {
            "ready"
        };
        let hearing = if self.deaf { " · not hearing" } else { "" };
        format!(
            "{} · {} · {work}{hearing}",
            self.calling(&self.soul),
            self.calling(&self.strand),
        )
    }

    pub(crate) fn position(&self) -> Option<String> {
        if self.follow {
            return None;
        }
        let first = self.scroll.saturating_add(1);
        let last = self.scroll.saturating_add(self.viewport).min(self.total);
        let unseen = if self.unseen { " ↑new" } else { "" };
        Some(format!("{first}-{last}/{}{unseen}", self.total))
    }

    pub(crate) fn footer(&self) -> String {
        let hint = if self.follow {
            "Enter send · Shift-Enter newline · drag to copy · Ctrl-T detail"
        } else {
            "PgDown/Ctrl-End bottom · drag to copy · Ctrl-T detail"
        };
        let live = self
            .alive()
            .map(|held| format!(" · {held}"))
            .unwrap_or_default();
        match self.position() {
            Some(position) => format!("{}{live} · {position} · {hint}", self.context),
            None => format!("{}{live} · {hint}", self.context),
        }
    }
}

pub(super) fn short(id: &str) -> String {
    match id.split_once('_') {
        Some((kind, rest)) if rest.len() > 8 => format!("{kind}_{}", &rest[..8]),
        _ => id.to_string(),
    }
}

impl State {}
