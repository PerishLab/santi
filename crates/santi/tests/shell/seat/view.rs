use super::Stroke;
use super::{Beat, Kind, Step, drawn, fresh};

#[test]
fn position() {
    let mut state = fresh();
    state.viewport(100, 10);
    assert!(
        state.position().is_none(),
        "the bottom is the one position that needs no reporting"
    );

    state.struck(Some(Stroke::PageUp));
    state.viewport(100, 10);
    assert_eq!(
        state.position().as_deref(),
        Some("86-95/100"),
        "paging moves half a screen so the previous view keeps an anchor"
    );

    state.absorb(Beat::Speech("output while the reader is away".to_string()));
    state.viewport(101, 10);
    assert_eq!(
        state.position().as_deref(),
        Some("86-95/101 ↑new"),
        "new output must not silently pass an away reader"
    );

    state.struck(Some(Stroke::Bottom));
    state.viewport(101, 10);
    assert!(
        state.position().is_none(),
        "returning to the bottom clears both the position and the unseen mark"
    );
}

#[test]
fn holds() {
    let mut state = fresh();
    state.viewport(20, 5);
    assert_eq!(state.scroll, 15);
    state.struck(Some(Stroke::PageUp));
    let held = state.scroll;
    state.absorb(Beat::Speech("new output".into()));
    state.viewport(21, 5);
    assert_eq!(state.scroll, held);
    assert!(
        state.footer().contains("bottom"),
        "a reader who scrolled away is told how to get back"
    );
    assert!(state.top().contains("ready"));
}

#[test]
fn drag() {
    let mut state = drawn(&["first line", "second line", "third line"]);

    state.struck(Some(Stroke::Press(6, 0)));
    state.struck(Some(Stroke::Drag(5, 2)));
    let Step::Copy(text) = state.struck(Some(Stroke::Release)) else {
        panic!("releasing the drag is what copies");
    };

    assert_eq!(
        text, "line\nsecond line\nthird",
        "the ends are partial and the middle is whole"
    );
    assert!(
        state.highlight(0).is_none(),
        "the highlight clears once the copy is taken"
    );
}

#[test]
fn backwards() {
    let mut forward = drawn(&["alpha", "beta"]);
    forward.struck(Some(Stroke::Press(2, 0)));
    forward.struck(Some(Stroke::Drag(3, 1)));

    let mut backward = drawn(&["alpha", "beta"]);
    backward.struck(Some(Stroke::Press(3, 1)));
    backward.struck(Some(Stroke::Drag(2, 0)));

    assert_eq!(
        forward.struck(Some(Stroke::Release)),
        backward.struck(Some(Stroke::Release)),
        "direction of the gesture is not part of what it means"
    );
}

#[test]
fn boundary() {
    let mut state = drawn(&["你好abc"]);
    state.struck(Some(Stroke::Press(0, 0)));
    state.struck(Some(Stroke::Drag(3, 0)));
    let Step::Copy(text) = state.struck(Some(Stroke::Release)) else {
        panic!("a drag over wide characters still copies");
    };

    assert_eq!(
        text, "你好",
        "an edge inside a wide character takes the whole character, never half"
    );
}

#[test]
fn click() {
    let mut state = drawn(&["line"]);
    state.struck(Some(Stroke::Press(2, 0)));

    assert_eq!(
        state.struck(Some(Stroke::Release)),
        Step::Stay,
        "an empty selection must not replace the clipboard with emptiness"
    );
    assert!(state.highlight(0).is_none());
}

#[test]
fn offset() {
    let mut state = fresh();
    let body = (0..30).map(|n| format!("line {n}")).collect::<Vec<_>>();
    state.rendered(body, 1, 5);
    assert_eq!(state.scroll, 25, "following holds the bottom");

    state.struck(Some(Stroke::Press(0, 1)));
    state.struck(Some(Stroke::Drag(7, 1)));
    let Step::Copy(text) = state.struck(Some(Stroke::Release)) else {
        panic!("copies");
    };
    assert_eq!(text, "line 25");
}

#[test]
fn whole() {
    let mut state = fresh();
    state.absorb(Beat::Speech("soul> hello".into()));
    state.absorb(Beat::Event(Kind::Tool, None, "tool call read_file".into()));

    let Step::Copy(text) = state.struck(Some(Stroke::Copy)) else {
        panic!("copy must yield the text rather than touch the terminal");
    };
    assert!(text.contains("soul> hello"));
    assert!(
        text.contains("tool call read_file"),
        "a collapsed group is still copied in full"
    );
}
