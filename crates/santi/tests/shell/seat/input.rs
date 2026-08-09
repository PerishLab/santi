use super::Stroke;
use super::{Beat, Step, enter, fresh};

#[test]
fn paste() {
    let mut state = fresh();
    state.struck(Some(Stroke::Paste("  one\n二🙂\tthree  ".to_string())));
    assert_eq!(
        state.struck(Some(Stroke::Enter)),
        Step::Speak("  one\n二🙂\tthree  ".to_string())
    );
    assert!(state.typed.is_empty());
}

#[test]
fn command() {
    let mut state = fresh();
    assert_eq!(
        enter(&mut state, " /status "),
        Step::Speak(" /status ".into())
    );
    state.absorb(Beat::Broken("done".into()));
    assert_eq!(
        enter(&mut state, "/status\nexplain"),
        Step::Speak("/status\nexplain".into())
    );
}

#[test]
fn grapheme() {
    let mut state = fresh();
    state.insert("a🙂e\u{301}z");
    state.left();
    state.erase();
    assert_eq!(state.typed, "a🙂z");
    state.home();
    state.right();
    state.delete();
    assert_eq!(state.typed, "az");
}

#[test]
fn vertical() {
    let mut state = fresh();
    state.insert("abc\n你x\nlast");
    state.end();
    state.up();
    assert_eq!(&state.typed[state.cursor..], "\nlast");
    state.up();
    assert_eq!(&state.typed[state.cursor..], "\n你x\nlast");
    state.down();
    assert_eq!(&state.typed[state.cursor..], "\nlast");
}
