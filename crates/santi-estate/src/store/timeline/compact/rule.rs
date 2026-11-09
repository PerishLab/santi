use super::thread::Thread;

pub(super) enum Rule {
    Exchange { start: i64, end: Option<i64> },
    Anchor { at: i64, text: Option<i64> },
    Broken { start: i64, end: Option<i64> },
}

enum Block {
    Clear,
    Hole(i64, i64),
    Dead,
}

impl Rule {
    fn block(&self, from: i64) -> Block {
        match *self {
            Rule::Exchange { start, end } => {
                let end = end.unwrap_or(i64::MAX);
                if from > end {
                    Block::Clear
                } else if from > start {
                    Block::Dead
                } else {
                    Block::Hole(start, end - 1)
                }
            }
            Rule::Anchor { at, text } => {
                if from > at {
                    Block::Clear
                } else {
                    Block::Hole(at, text.map_or(i64::MAX, |text| text - 1))
                }
            }
            Rule::Broken { start, end } => {
                let end = end.unwrap_or(i64::MAX);
                if from > end {
                    Block::Clear
                } else if from > start {
                    Block::Dead
                } else {
                    Block::Hole(start, i64::MAX)
                }
            }
        }
    }

    fn explain(&self, from: i64, to: i64) -> String {
        match *self {
            Rule::Exchange {
                start,
                end: Some(end),
            } => format!(
                "[compact.exchange] compact {from}..{to} would cut or hide part of the tool exchange recorded at timeline positions {start} through {end}; enclose the whole exchange with eligible message boundaries (first/last or from/to naming fixed messages) lying before its reasoning anchor and after its last matching result, or keep the range entirely outside it. Those timeline positions are not message selectors"
            ),
            Rule::Exchange { start, end: None } => format!(
                "[compact.exchange] compact {from}..{to} touches the tool exchange recorded from timeline position {start}, whose declared calls, results or text are not all recorded yet; an unfinished or unprovable exchange cannot be hidden, so end the range at an eligible message boundary before it or wait until it is complete. That timeline position is not a message selector"
            ),
            Rule::Anchor {
                at,
                text: Some(text),
            } => format!(
                "[compact.anchor] compact {from}..{to} would hide the reasoning anchor at timeline position {at} while its assistant text message at timeline position {text} stays live; end at an eligible message boundary before the anchor or include that message. Those timeline positions are not message selectors"
            ),
            Rule::Anchor { at, text: None } => format!(
                "[compact.anchor] compact {from}..{to} would hide the reasoning anchor at timeline position {at} while its declared assistant text is not recorded yet; end at an eligible message boundary before the anchor or wait for the text. That timeline position is not a message selector"
            ),
            Rule::Broken {
                start,
                end: Some(end),
            } => format!(
                "[compact.exchange] compact {from}..{to} overlaps the exchange recorded at timeline positions {start} through {end}, whose calls, results or continuation state do not agree; that history cannot be collapsed safely, so choose eligible message boundaries entirely before or after it. Those timeline positions are not message selectors"
            ),
            Rule::Broken { start, end: None } => format!(
                "[compact.exchange] compact {from}..{to} overlaps the exchange recorded from timeline position {start}, whose declared calls, results, text or continuation state are not all recorded yet; an unfinished or unprovable exchange cannot be hidden, so end at an eligible message boundary before it or wait until it is complete. That timeline position is not a message selector"
            ),
        }
    }
}

impl Thread {
    pub(crate) fn split(&self, from: i64, to: i64) -> Option<String> {
        self.rules.iter().find_map(|rule| match rule.block(from) {
            Block::Dead => Some(rule.explain(from, to)),
            Block::Hole(low, high) if low <= to && to <= high => Some(rule.explain(from, to)),
            _ => None,
        })
    }

    pub(crate) fn holes(&self, from: i64) -> Option<Vec<(i64, i64)>> {
        let mut holes = Vec::new();
        for rule in &self.rules {
            match rule.block(from) {
                Block::Clear => {}
                Block::Hole(low, high) => holes.push((low, high)),
                Block::Dead => return None,
            }
        }
        Some(holes)
    }
}
