use unicode_width::UnicodeWidthStr;

use super::{Span, State, offset};

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
                offset(line, start.1)
            } else {
                0
            };
            let to = if row == end.0 {
                offset(line, end.1)
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
            "soul: {} · strand: {} · {work}{hearing}",
            short(&self.soul),
            short(&self.strand),
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
        match self.position() {
            Some(position) => format!("{} · {position} · {hint}", self.context),
            None => format!("{} · {hint}", self.context),
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
