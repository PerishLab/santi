#[allow(dead_code)]
mod keys {
    pub(super) enum Stroke {
        Enter,
        Newline,
        Paste(String),
        Erase,
        Delete,
        Left,
        Right,
        Home,
        End,
        WordLeft,
        WordRight,
        Up,
        Down,
        ScrollUp,
        ScrollDown,
        PageUp,
        PageDown,
        Bottom,
        Verbose,
        Copy,
        Press(u16, u16),
        Drag(u16, u16),
        Release,
        Interrupt,
        Quit,
        Typed(char),
        Redraw,
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Thinking,
    Tool,
    Turn,
    Fault,
    Other,
}

#[allow(dead_code)]
#[path = "../../src/client/tui/state.rs"]
mod state;

use keys::Stroke;
use state::recover::recover;
use state::{Beat, Entry, State, Step};

fn fresh() -> State {
    State::new("soul_luna".to_string(), "ss_direct".to_string())
}

fn enter(state: &mut State, text: &str) -> Step {
    state.typed = text.to_string();
    state.cursor = text.len();
    state.struck(Some(Stroke::Enter))
}

fn drawn(lines: &[&str]) -> State {
    let mut state = fresh();
    let body = lines.iter().map(|line| (*line).to_string()).collect();
    state.rendered(body, 0, lines.len().max(1));
    state
}

#[path = "seat/basic.rs"]
mod basic;
#[path = "seat/input.rs"]
mod input;
#[path = "seat/stream.rs"]
mod stream;
#[path = "seat/view.rs"]
mod view;
