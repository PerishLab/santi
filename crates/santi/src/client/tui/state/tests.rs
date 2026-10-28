use super::super::keys::Stroke;
use super::recover::recover;
use super::{Beat, Kind, State, Step};

pub(super) fn fresh() -> State {
    State::new(
        "soul_luna".to_string(),
        "ss_direct".to_string(),
        Default::default(),
    )
}

pub(super) fn enter(state: &mut State, text: &str) -> Step {
    state.typed = text.to_string();
    state.cursor = text.len();
    state.struck(Some(Stroke::Enter))
}

pub(super) fn drawn(lines: &[&str]) -> State {
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

fn active() -> State {
    let mut state = fresh();
    assert!(matches!(enter(&mut state, "run"), Step::Speak(_)));
    state.absorb(Beat::Accepted("inbox_current".into()));
    state.absorb(Beat::Event(
        Kind::Turn,
        Some("turn_current_long".into()),
        "started".into(),
    ));
    state
}

#[test]
fn settles() {
    let mut state = active();
    assert!(state.footer().contains("Esc interrupt"));
    assert_eq!(
        state.struck(Some(Stroke::Interrupt)),
        Step::Stop("turn_current_long".into())
    );
    assert!(state.top().contains("interrupt requested"));
    assert_eq!(state.struck(Some(Stroke::Interrupt)), Step::Stay);
    state.stopped("turn_current_long".into(), Ok(true));
    assert!(state.busy && state.top().contains("interrupt accepted"));
    state.absorb(Beat::Event(
        Kind::Interrupted,
        Some("turn_current_long".into()),
        "runtime.turn.interrupted: interrupted by operator".into(),
    ));
    state.absorb(Beat::Unsettled("receipt is still being checked".into()));
    assert_eq!(enter(&mut state, "continue"), Step::Stay);
    assert_eq!(state.typed, "continue");
    assert!(state.busy);
    assert_eq!(
        state.heard(Some(Beat::Failed(
            "inbox_current".into(),
            Some("turn_current_long".into()),
            "durably failed; do not resend".into()
        ))),
        Step::Refresh
    );
    assert!(!state.busy && !state.unsettled && state.stop.is_none());
    let quiet = screen(&mut state);
    assert!(quiet.contains("Interrupted. Send a new instruction"));
    assert!(quiet.contains("prior effects are retained"));
    assert!(!quiet.contains("runtime.turn.interrupted"));
    assert!(!quiet.contains("inbox_current"));
    assert!(!quiet.contains("do not resend"));
    state.struck(Some(Stroke::Verbose));
    let detail = screen(&mut state);
    assert!(detail.contains("runtime.turn.interrupted"));
    assert!(detail.contains("inbox_current"));
    assert!(detail.contains("do not resend"));
    assert!(matches!(state.struck(Some(Stroke::Enter)), Step::Speak(text) if text == "continue"));
    assert!(state.turn.is_none());
    assert_eq!(state.struck(Some(Stroke::Interrupt)), Step::Stay);
}

#[test]
fn failed() {
    for (kind, event_turn, receipt_turn) in [
        (Kind::Fault, "turn_current_long", Some("turn_current_long")),
        (Kind::Interrupted, "turn_other", Some("turn_current_long")),
        (Kind::Interrupted, "turn_current_long", None),
    ] {
        let mut state = active();
        state.struck(Some(Stroke::Interrupt));
        state.stopped("turn_current_long".into(), Ok(true));
        state.absorb(Beat::Event(
            kind,
            Some(event_turn.into()),
            "original diagnostic".into(),
        ));
        state.heard(Some(Beat::Failed(
            "inbox_current".into(),
            receipt_turn.map(str::to_string),
            "durably failed".into(),
        )));
        assert!(state.transcript().contains("Task failed."));
        assert!(!state.transcript().contains("Interrupted. Send"));
        assert!(state.transcript().contains("prior effects may remain"));
    }
}

#[test]
fn late() {
    let mut state = active();
    state.struck(Some(Stroke::Interrupt));
    state.absorb(Beat::Settled("inbox_current".into()));
    state.stopped("turn_current_long".into(), Ok(true));
    assert!(!state.busy && state.stop.is_none());
    assert!(state.top().contains("ready"));
    assert!(!state.transcript().contains("Interrupted. Send"));
}

#[test]
fn unconfirmed() {
    let mut state = active();
    state.struck(Some(Stroke::Interrupt));
    state.stopped("turn_current_long".into(), Err("network error".into()));
    assert!(state.busy);
    assert!(state.transcript().contains("may still be running"));
    assert_eq!(enter(&mut state, "next"), Step::Stay);
    state.stopped("turn_current_long".into(), Ok(false));
    assert!(state.busy && state.stop.is_none());
    assert_eq!(enter(&mut state, "next"), Step::Stay);
}

fn screen(state: &mut State) -> String {
    let backend = ratatui::backend::TestBackend::new(140, 35);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut cache = super::super::paint::Cache::new();
    terminal
        .draw(|frame| super::super::paint::draw(frame, state, &mut cache))
        .unwrap();
    terminal
        .backend()
        .buffer()
        .content()
        .chunks(140)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn unrelated() {
    let mut state = active();
    state.absorb(Beat::Failed(
        "other_receipt".into(),
        Some("turn_other".into()),
        "failed".into(),
    ));
    assert!(state.busy);
    assert_eq!(state.receipt.as_deref(), Some("inbox_current"));
    assert!(!state.transcript().contains("Interrupted. Send"));
}
