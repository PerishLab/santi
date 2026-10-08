use super::*;
use crate::watch::Bytes;

fn activity(turn: &str, state: &str) -> String {
    serde_json::json!({"payload":{"beat":"active","activity":{"turn":turn,"state":state}}})
        .to_string()
}

fn shown(presentation: Presentation, frames: &[(&str, String)]) -> String {
    let mut display = Display::new(presentation);
    let mut output = Bytes(Vec::new());
    for (event, data) in frames {
        display.write(&mut output, event, data);
    }
    display.finish(&mut output);
    String::from_utf8(output.0).expect("text")
}

#[test]
fn transitions() {
    let frames = [
        ("turn", activity("t1", "requesting")),
        ("turn", activity("t1", "thinking")),
        ("turn", activity("t1", "thinking")),
        ("turn", activity("t1", "calling")),
        ("turn", activity("t1", "running")),
        ("turn", activity("t1", "running")),
        ("turn", activity("t1", "requesting")),
        ("turn", activity("t1", "thinking")),
        ("turn", activity("t1", "generating")),
    ];
    assert_eq!(
        shown(Presentation::Watch(WatchFormat::Filtered), &frames),
        "turn t1: requesting\nturn t1: thinking\nturn t1: calling\nturn t1: running\nturn t1: requesting\nturn t1: thinking\nturn t1: generating\n"
    );
}

#[test]
fn milestones() {
    let frames = [
        ("turn", activity("t1", "thinking")),
        (
            "thinking",
            r#"{"payload":{"beat":"created","thinking":{"id":"r1","turn":"t1"}}}"#.into(),
        ),
        ("turn", activity("t1", "thinking")),
        (
            "tool",
            r#"{"payload":{"beat":"called","call":{"id":"c1","tool":"shell"}}}"#.into(),
        ),
        (
            "message",
            r#"{"payload":{"beat":"completed","turn":"t1","message":{"text":"OK"}}}"#.into(),
        ),
        (
            "turn",
            r#"{"payload":{"beat":"completed","turn":"t1"}}"#.into(),
        ),
    ];
    assert_eq!(
        shown(Presentation::Watch(WatchFormat::Filtered), &frames),
        "turn t1: thinking\nthinking started r1 (t1)\ntool call shell (c1)\nassistant completed t1: OK\nturn completed t1\n"
    );
}

#[test]
fn identity() {
    let frames = [
        ("turn", activity("t1", "thinking")),
        ("turn", activity("t2", "thinking")),
        ("turn", activity("t1", "thinking")),
    ];
    assert_eq!(
        shown(Presentation::Watch(WatchFormat::Filtered), &frames),
        "turn t1: thinking\nturn t2: thinking\nturn t1: thinking\n"
    );
}

#[test]
fn reset() {
    for frame in [
        ("gap", r#"{"payload":{"from":"e1"}}"#.into()),
        ("turn", r#"{"payload":{"beat":"started","turn":{"id":"t1"}}}"#.into()),
        ("turn", r#"{"payload":{"beat":"completed","turn":"t1"}}"#.into()),
        ("turn", r#"{"payload":{"beat":"failed","turn":"t1","error":{"code":"fault","message":"failed"}}}"#.into()),
    ] {
        let frames = [
            ("turn", activity("t1", "thinking")),
            frame,
            ("turn", activity("t1", "thinking")),
        ];
        let output = shown(Presentation::Watch(WatchFormat::Filtered), &frames);
        assert_eq!(output.matches("turn t1: thinking\n").count(), 2);
        assert_eq!(output.lines().count(), 3);
    }
}

#[test]
fn unknown() {
    for data in [
        activity("t1", "future"),
        activity("t1", "unknown"),
        r#"{"payload":{"beat":"active","activity":{"turn":"t1"}}}"#.into(),
    ] {
        let frames = [("turn", data.clone()), ("turn", data)];
        let output = shown(Presentation::Watch(WatchFormat::Filtered), &frames);
        assert_eq!(output.lines().count(), 2);
    }
}

#[test]
fn malformed() {
    for data in [
        r#"{"payload":{"beat":"active"}}"#,
        r#"{"payload":{"beat":"active","activity":{"state":"thinking"}}}"#,
    ] {
        let frames = [
            ("turn", activity("t1", "thinking")),
            ("turn", data.into()),
            ("turn", activity("t1", "thinking")),
        ];
        assert_eq!(
            shown(Presentation::Watch(WatchFormat::Filtered), &frames),
            "turn t1: thinking\nturn t1: thinking\n"
        );
    }
}

#[test]
fn raw() {
    let data = activity("t1", "thinking");
    let frames = [("turn", data.clone()), ("turn", data.clone())];
    assert_eq!(
        shown(Presentation::Watch(WatchFormat::Raw), &frames),
        format!("{data}\n{data}\n")
    );
    assert_eq!(
        shown(Presentation::Tui, &frames),
        "turn t1: thinking\nturn t1: thinking\n"
    );
}
