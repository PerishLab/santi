use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Editor {
    pub(super) lines: Vec<String>,
    pub(super) row: usize,
    pub(super) column: usize,
    pub(super) top: usize,
    pub(super) total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Visual {
    text: String,
    start: usize,
    end: usize,
}

struct Wrap<'a> {
    source: &'a str,
    width: usize,
}

const PROMPT: &str = "❯ ";
const CONTINUATION: &str = "  ";

pub(super) fn editor(source: &str, cursor: usize, width: usize, height: usize) -> Editor {
    let width = width.max(1);
    let height = height.max(1);
    let visual = Wrap {
        source,
        width: width.saturating_sub(PROMPT.width()).max(1),
    }
    .visuals();
    let line = visual
        .iter()
        .position(|held| cursor >= held.start && cursor <= held.end)
        .unwrap_or_else(|| visual.len().saturating_sub(1));
    let column = prefix(line) + UnicodeWidthStr::width(&source[visual[line].start..cursor]);
    let total = visual.len().max(1);
    let visible = total.min(height);
    let top = line.saturating_sub(visible - 1).min(total - visible);
    let lines = visual
        .iter()
        .skip(top)
        .take(visible)
        .enumerate()
        .map(|(seen, held)| {
            let marker = if top + seen == 0 {
                PROMPT
            } else {
                CONTINUATION
            };
            format!("{marker}{}", held.text)
        })
        .collect();

    Editor {
        lines,
        row: line - top,
        column,
        top,
        total,
    }
}

pub(super) fn text(source: &str, width: usize) -> Vec<String> {
    Wrap {
        source,
        width: width.max(1),
    }
    .visuals()
    .into_iter()
    .map(|held| held.text)
    .collect()
}

fn prefix(line: usize) -> usize {
    if line == 0 {
        PROMPT.width()
    } else {
        CONTINUATION.width()
    }
}

impl Wrap<'_> {
    fn visuals(&self) -> Vec<Visual> {
        let mut out = Vec::new();
        let mut start = 0;
        loop {
            let rest = &self.source[start..];
            let (end, more) = match rest.find('\n') {
                Some(index) => (start + index, true),
                None => (self.source.len(), false),
            };
            self.part(start, end, &mut out);
            if !more {
                break;
            }
            start = end + 1;
            if start == self.source.len() {
                out.push(Visual {
                    text: String::new(),
                    start,
                    end: start,
                });
                break;
            }
        }
        if out.is_empty() {
            out.push(Visual {
                text: String::new(),
                start: 0,
                end: 0,
            });
        }
        out
    }

    fn part(&self, start: usize, end: usize, out: &mut Vec<Visual>) {
        if start == end {
            out.push(Visual {
                text: String::new(),
                start,
                end,
            });
            return;
        }
        let mut begin = start;
        let mut stop = start;
        let mut used = 0;
        for (offset, grapheme) in self.source[start..end].grapheme_indices(true) {
            let absolute = start + offset;
            let cost = UnicodeWidthStr::width(grapheme);
            if stop > begin && used + cost > self.width {
                out.push(Visual {
                    text: self.source[begin..stop].to_string(),
                    start: begin,
                    end: stop,
                });
                begin = absolute;
                used = 0;
            }
            stop = absolute + grapheme.len();
            used += cost;
        }
        out.push(Visual {
            text: self.source[begin..stop].to_string(),
            start: begin,
            end: stop,
        });
    }
}
