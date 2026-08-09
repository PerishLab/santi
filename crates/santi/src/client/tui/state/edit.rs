use unicode_width::UnicodeWidthStr;

use super::glyph::{self, glyph};
use super::{DRAFT, State};

impl State {
    pub(crate) fn insert(&mut self, text: &str) {
        if self.typed.len().saturating_add(text.len()) > DRAFT {
            self.push(format!(
                "draft refused: local safety limit is {DRAFT} bytes; nothing was inserted"
            ));
            return;
        }
        self.typed.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.preferred = None;
    }

    pub(crate) fn erase(&mut self) {
        let Some(start) = glyph(&self.typed).before(self.cursor) else {
            return;
        };
        self.typed.drain(start..self.cursor);
        self.cursor = start;
        self.preferred = None;
    }

    pub(crate) fn delete(&mut self) {
        let Some(end) = glyph(&self.typed).after(self.cursor) else {
            return;
        };
        self.typed.drain(self.cursor..end);
        self.preferred = None;
    }

    pub(crate) fn left(&mut self) {
        if let Some(cursor) = glyph(&self.typed).before(self.cursor) {
            self.cursor = cursor;
        }
        self.preferred = None;
    }

    pub(crate) fn right(&mut self) {
        if let Some(cursor) = glyph(&self.typed).after(self.cursor) {
            self.cursor = cursor;
        }
        self.preferred = None;
    }

    pub(crate) fn leftward(&mut self) {
        let mut cursor = self.cursor;
        while let Some(start) = glyph(&self.typed).before(cursor) {
            cursor = start;
            if glyph::word(&self.typed[cursor..]) {
                break;
            }
        }
        while let Some(start) = glyph(&self.typed).before(cursor) {
            if !glyph::word(&self.typed[start..cursor]) {
                break;
            }
            cursor = start;
        }
        self.cursor = cursor;
        self.preferred = None;
    }

    pub(crate) fn rightward(&mut self) {
        let mut cursor = self.cursor;
        while let Some(end) = glyph(&self.typed).after(cursor) {
            if glyph::word(&self.typed[cursor..end]) {
                break;
            }
            cursor = end;
        }
        while let Some(end) = glyph(&self.typed).after(cursor) {
            if !glyph::word(&self.typed[cursor..end]) {
                break;
            }
            cursor = end;
        }
        self.cursor = cursor;
        self.preferred = None;
    }

    pub(crate) fn ahead(&mut self) {
        let start = glyph(&self.typed).start(self.cursor);
        self.typed.drain(start..self.cursor);
        self.cursor = start;
        self.preferred = None;
    }

    pub(crate) fn behind(&mut self) {
        let end = glyph(&self.typed).end(self.cursor);
        self.typed.drain(self.cursor..end);
        self.preferred = None;
    }

    pub(crate) fn shed(&mut self) {
        let from = self.cursor;
        self.leftward();
        self.typed.drain(self.cursor..from);
        self.preferred = None;
    }

    pub(crate) fn home(&mut self) {
        self.cursor = glyph(&self.typed).start(self.cursor);
        self.preferred = None;
    }

    pub(crate) fn end(&mut self) {
        self.cursor = glyph(&self.typed).end(self.cursor);
        self.preferred = None;
    }

    pub(crate) fn up(&mut self) {
        self.vertical(false);
    }

    pub(crate) fn down(&mut self) {
        self.vertical(true);
    }

    pub(super) fn vertical(&mut self, down: bool) {
        let start = glyph(&self.typed).start(self.cursor);
        let end = glyph(&self.typed).end(self.cursor);
        let column = self
            .preferred
            .unwrap_or_else(|| UnicodeWidthStr::width(&self.typed[start..self.cursor]));
        let target = if down {
            if end == self.typed.len() {
                return;
            }
            let next = end + 1;
            (next, glyph(&self.typed).end(next))
        } else {
            if start == 0 {
                return;
            }
            let previous = start - 1;
            (glyph(&self.typed).start(previous), previous)
        };
        self.cursor = glyph(&self.typed[target.0..target.1]).column(target.0, column);
        self.preferred = Some(column);
    }

    pub(super) fn take(&mut self) -> Option<String> {
        if self.typed.trim().is_empty() {
            return None;
        }
        self.cursor = 0;
        self.preferred = None;
        Some(std::mem::take(&mut self.typed))
    }
}
