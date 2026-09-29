use super::super::keys::Stroke;
use super::recover::recover;
use super::{Beat, Entry, Kind, State, Step};

fn fresh() -> State {
    State::new(
        "soul_luna".to_string(),
        "ss_direct".to_string(),
        Default::default(),
    )
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

#[path = "basic.rs"]
mod basic;
#[path = "input.rs"]
mod input;
#[path = "pane.rs"]
mod pane;
#[path = "stream.rs"]
mod stream;
