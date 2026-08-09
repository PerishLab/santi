use super::{Beat, State, Step, enter, fresh, recover};
use std::io;

#[test]
fn reload() {
    let mut state = fresh();
    assert_eq!(enter(&mut state, "/reload"), Step::Reload);
    assert!(state.typed.is_empty());
}

#[test]
fn active() {
    let mut state = fresh();
    assert_eq!(
        enter(&mut state, "first message"),
        Step::Speak("first message".to_string())
    );
    assert!(state.top().contains("sending"));

    assert_eq!(enter(&mut state, "/reload"), Step::Stay);
    assert_eq!(enter(&mut state, "second message"), Step::Stay);
    assert_eq!(state.typed, "second message");
    let spoken = state.transcript();
    assert!(spoken.contains("reload refused"));
    assert!(spoken.contains("draft is preserved"));
}

#[test]
fn unsettled() {
    let mut state = fresh();
    assert!(matches!(enter(&mut state, "first message"), Step::Speak(_)));
    assert_eq!(
        state.heard(Some(Beat::Unsettled("state unknown".to_string()))),
        Step::Stay
    );
    assert!(state.top().contains("receipt unsettled"));

    assert_eq!(enter(&mut state, "/reload"), Step::Stay);
    assert_eq!(enter(&mut state, "do not resend"), Step::Stay);
    assert_eq!(state.typed, "do not resend");
    assert!(state.transcript().contains("not durably settled"));
}

#[test]
fn completed() {
    let mut state = fresh();
    state.absorb(Beat::Unsettled("state unknown".to_string()));
    assert_eq!(
        state.heard(Some(Beat::Settled("inbox_done".to_string()))),
        Step::Refresh
    );
    assert!(state.top().contains("ready"));
    assert_eq!(enter(&mut state, "/reload"), Step::Reload);
}

#[test]
fn failed() {
    let mut state = fresh();
    assert!(matches!(enter(&mut state, "message"), Step::Speak(_)));
    assert_eq!(
        state.heard(Some(Beat::Broken("not accepted".to_string()))),
        Step::Refresh
    );
    assert!(state.top().contains("ready"));
    assert_eq!(enter(&mut state, "/reload"), Step::Reload);
}

#[test]
fn status() {
    let mut state = fresh();
    assert_eq!(enter(&mut state, "/status"), Step::Refresh);
    assert!(state.top().contains("ready"));
    assert!(state.footer().contains("context: refreshing"));
    assert_eq!(enter(&mut state, "/reload"), Step::Reload);
}

#[test]
fn restores() {
    let mut state = State::new("soul_luna".to_string(), "ss_direct".to_string());
    let mut restored = false;
    let error = recover(
        &mut state,
        anyhow::anyhow!("exec failed"),
        || Err::<(), _>(io::Error::other("init failed")),
        || restored = true,
    )
    .expect_err("failed reinit must be returned");

    assert!(restored);
    let error = format!("{error:#}");
    assert!(error.contains("restore old TUI after reload failure"));
    assert!(error.contains("init failed"));
    assert!(state.transcript().contains("exec failed"));
}

#[test]
fn visible() {
    let mut state = State::new("soul_luna".to_string(), "ss_direct".to_string());
    let mut restored = false;
    recover(
        &mut state,
        anyhow::anyhow!("restore terminal failed"),
        || Ok::<_, io::Error>(()),
        || restored = true,
    )
    .expect("old TUI reinit");

    assert!(!restored);
    let spoken = state.transcript();
    assert!(spoken.contains("reload failed; still running the old TUI"));
    assert!(spoken.contains("restore terminal failed"));
}
