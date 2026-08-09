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
    let mut state = State::new(
        "soul_luna".to_string(),
        "ss_direct".to_string(),
        Default::default(),
    );
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
    let mut state = State::new(
        "soul_luna".to_string(),
        "ss_direct".to_string(),
        Default::default(),
    );
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

#[test]
fn named() {
    let mut state = fresh();
    assert!(
        state.top().contains("ss_direct"),
        "an unnamed strand falls back to its id"
    );

    let Step::Name(id, name) = enter(&mut state, "/alias main") else {
        panic!("aliasing yields the pair to persist, it does not write from state");
    };
    assert_eq!(name, "main");
    assert!(id.starts_with("ss_"));
    assert!(state.top().contains("main"), "the name shows at once");
    assert!(!state.top().contains("ss_direct"));

    let Step::Name(_, cleared) = enter(&mut state, "/alias") else {
        panic!("a bare /alias clears");
    };
    assert!(cleared.is_empty());
    assert!(
        state.top().contains("ss_direct"),
        "cleared falls back to the id"
    );
}

#[test]
fn travels() {
    let mut state = fresh();
    assert_eq!(
        enter(&mut state, "/strand"),
        Step::Listing("strands".to_string()),
        "a bare /strand lists rather than guessing"
    );
    assert_eq!(
        enter(&mut state, "/soul"),
        Step::Listing("souls".to_string())
    );

    let Step::Switch(next) = enter(&mut state, "/strand ss_other") else {
        panic!("naming a strand switches to it");
    };
    assert_eq!(next, "ss_other");

    assert_eq!(
        enter(&mut state, "/strand ss_direct"),
        Step::Stay,
        "switching to the current strand is a no-op, not a reload"
    );
    assert!(state.transcript().contains("already on"));
}

#[test]
fn aliased() {
    let mut state = fresh();
    let Step::Name(..) = enter(&mut state, "/alias 主线") else {
        panic!("alias set");
    };
    assert_eq!(
        enter(&mut state, "/strand 主线"),
        Step::Stay,
        "an alias resolves to its id before the switch is judged"
    );
}
