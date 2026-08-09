use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) struct Glyph<'a> {
    pub(super) text: &'a str,
}

pub(super) fn glyph(text: &str) -> Glyph<'_> {
    Glyph { text }
}

pub(super) fn word(grapheme: &str) -> bool {
    grapheme
        .chars()
        .next()
        .is_some_and(|character| character.is_alphanumeric() || character == '_')
}

impl Glyph<'_> {
    pub(super) fn before(&self, cursor: usize) -> Option<usize> {
        self.text[..cursor]
            .grapheme_indices(true)
            .next_back()
            .map(|(index, _)| index)
    }

    pub(super) fn after(&self, cursor: usize) -> Option<usize> {
        self.text[cursor..]
            .graphemes(true)
            .next()
            .map(|grapheme| cursor + grapheme.len())
    }

    pub(super) fn start(&self, cursor: usize) -> usize {
        self.text[..cursor].rfind('\n').map_or(0, |index| index + 1)
    }

    pub(super) fn end(&self, cursor: usize) -> usize {
        self.text[cursor..]
            .find('\n')
            .map_or(self.text.len(), |index| cursor + index)
    }

    pub(super) fn column(&self, offset: usize, column: usize) -> usize {
        let mut cursor = offset;
        let mut used = 0;
        for grapheme in self.text.graphemes(true) {
            let width = UnicodeWidthStr::width(grapheme);
            if used + width > column {
                break;
            }
            used += width;
            cursor += grapheme.len();
        }
        cursor
    }
}
