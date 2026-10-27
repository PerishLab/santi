use std::collections::VecDeque;
use std::path::PathBuf;

use tokio::io::{AsyncRead, AsyncReadExt};

use super::redact;

const SAVED: usize = 1024 * 1024;

pub(in crate::service::tools) struct Bounds {
    pub(in crate::service::tools) timeout: std::time::Duration,
    pub(in crate::service::tools) bytes: Option<usize>,
    pub(in crate::service::tools) lines: Option<usize>,
    pub(in crate::service::tools) saved: Option<Saved>,
}

pub(in crate::service::tools) struct Saved {
    pub(in crate::service::tools) path: PathBuf,
    pub(in crate::service::tools) uri: String,
}

impl Saved {
    pub(in crate::service::tools) fn at(home: PathBuf, call: &str) -> Self {
        let name = call
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        Self {
            path: home.join("outputs").join(&name),
            uri: crate::housed(crate::STRANDSPACE, &format!("outputs/{name}")),
        }
    }
}

pub(super) struct Pipe {
    kept: Vec<u8>,
    tail: VecDeque<u8>,
    total: usize,
    breaks: usize,
    bounded: bool,
}

pub(super) struct Text {
    pub(super) value: String,
    pub(super) truncated: bool,
    pub(super) redactions: usize,
    pub(super) saved: Option<String>,
}

pub(super) async fn piped(
    mut reader: impl AsyncRead + Unpin,
    bounded: Option<usize>,
) -> Result<Pipe, String> {
    let mut pipe = Pipe {
        kept: Vec::new(),
        tail: VecDeque::new(),
        total: 0,
        breaks: 0,
        bounded: bounded.is_some(),
    };
    let ring = bounded.unwrap_or(0);
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        let chunk = &chunk[..read];
        pipe.total += read;
        pipe.breaks += chunk.iter().filter(|byte| **byte == b'\n').count();
        let room = if pipe.bounded {
            SAVED.saturating_sub(pipe.kept.len())
        } else {
            read
        };
        pipe.kept.extend_from_slice(&chunk[..room.min(read)]);
        if pipe.bounded {
            pipe.tail.extend(chunk);
            let excess = pipe.tail.len().saturating_sub(ring);
            pipe.tail.drain(..excess);
        }
    }
    Ok(pipe)
}

impl Pipe {
    fn lines(&self) -> usize {
        let open =
            self.kept.last().is_some_and(|byte| *byte != b'\n') || self.total > self.kept.len();
        self.breaks + usize::from(open && self.total > 0)
    }

    fn width(&self) -> usize {
        String::from_utf8_lossy(&self.kept).len() + (self.total - self.kept.len())
    }

    fn fit(&self, width: usize, bytes: usize, lines: usize) -> Fit {
        if !self.bounded {
            return Fit::Whole;
        }
        if self.total > self.kept.len() {
            return Fit::Cut;
        }
        if width <= bytes && self.lines() <= lines {
            Fit::Whole
        } else {
            Fit::Cut
        }
    }
}

enum Fit {
    Whole,
    Cut,
}

pub(super) fn shared(
    stdout: &Pipe,
    stderr: &Pipe,
    bytes: usize,
    lines: usize,
) -> [(usize, usize); 2] {
    let bytes = split(stdout.width(), stderr.width(), bytes);
    let lines = split(stdout.lines(), stderr.lines(), lines);
    [(bytes.0, lines.0), (bytes.1, lines.1)]
}

fn split(first: usize, second: usize, total: usize) -> (usize, usize) {
    if first.saturating_add(second) <= total {
        return (first, second);
    }
    let second = second.min((total / 2).max(total - first.min(total)));
    let first = (total - second).min(first);
    (first, total - first)
}

pub(super) fn rendered(
    pipe: Pipe,
    bytes: usize,
    lines: usize,
    saved: Option<(&Saved, &str)>,
) -> Text {
    let whole = String::from_utf8_lossy(&pipe.kept).into_owned();
    if let Fit::Whole = pipe.fit(whole.len(), bytes, lines) {
        let (value, redactions) = redact::text(&whole);
        return Text {
            value,
            truncated: false,
            redactions,
            saved: None,
        };
    }
    let (front, back) = (bytes / 2, bytes - bytes / 2);
    let (above, below) = (lines / 2, lines - lines / 2);
    let head = Cut(&whole).clipped(front).leading(above).0;
    let rest = pipe.tail.iter().copied().collect::<Vec<_>>();
    let rest = String::from_utf8_lossy(&rest);
    let tail = Cut(&rest).ending(back).trailing(below).0;
    let omitted = pipe.total.saturating_sub(head.len() + tail.len());
    let hidden = pipe
        .lines()
        .saturating_sub(Cut(head).count() + Cut(tail).count());
    let saved = saved.map(|(saved, stream)| kept(&pipe, &whole, saved, stream));
    let place = match &saved {
        Some(Ok(uri)) => format!("full output: {uri}"),
        Some(Err(error)) => format!("full output was not saved: {error}"),
        None => "full output was not saved".to_string(),
    };
    let (head, front) = redact::text(head);
    let (tail, back) = redact::text(tail);
    let gap = if head.is_empty() || head.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    Text {
        value: format!("{head}{gap}[… {omitted} bytes, {hidden} lines omitted; {place} …]\n{tail}"),
        truncated: true,
        redactions: front + back,
        saved: saved.and_then(Result::ok),
    }
}

fn kept(pipe: &Pipe, whole: &str, saved: &Saved, stream: &str) -> Result<String, String> {
    let path = saved.path.with_extension(format!("{stream}.txt"));
    let (mut text, _) = redact::text(whole);
    if pipe.total > pipe.kept.len() {
        text.push_str(&format!(
            "\n[… saved output stops at {} of {} bytes …]\n",
            pipe.kept.len(),
            pipe.total
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(&path, text).map_err(|error| error.to_string())?;
    Ok(format!("{}.{stream}.txt", saved.uri))
}

struct Cut<'a>(&'a str);

impl<'a> Cut<'a> {
    fn clipped(self, bytes: usize) -> Self {
        let mut end = bytes.min(self.0.len());
        while !self.0.is_char_boundary(end) {
            end -= 1;
        }
        Cut(&self.0[..end])
    }

    fn ending(self, bytes: usize) -> Self {
        let mut start = self.0.len().saturating_sub(bytes);
        while !self.0.is_char_boundary(start) {
            start += 1;
        }
        Cut(&self.0[start..])
    }

    fn leading(self, lines: usize) -> Self {
        if lines == 0 {
            return Cut("");
        }
        match self.0.match_indices('\n').nth(lines - 1) {
            Some((index, _)) => Cut(&self.0[..=index]),
            None => self,
        }
    }

    fn trailing(self, lines: usize) -> Self {
        if lines == 0 {
            return Cut("");
        }
        let body = self.0.strip_suffix('\n').unwrap_or(self.0);
        match body.rmatch_indices('\n').nth(lines - 1) {
            Some((index, _)) => Cut(&self.0[index + 1..]),
            None => self,
        }
    }

    fn count(&self) -> usize {
        self.0.matches('\n').count() + usize::from(!self.0.is_empty() && !self.0.ends_with('\n'))
    }
}
