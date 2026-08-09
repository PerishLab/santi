#[path = "../../src/client/tui/layout.rs"]
#[allow(dead_code)]
mod inner;

use inner::{editor, text};

#[test]
fn newlines() {
    assert_eq!(text("abc\ndefgh", 3), vec!["abc", "def", "gh"]);
    assert_eq!(text("a\n", 3), vec!["a", ""]);
}

#[test]
fn graphemes() {
    assert_eq!(text("你a界", 4), vec!["你a", "界"]);
    let composed = "e\u{301}x";
    assert_eq!(text(composed, 2), vec![composed]);
}

#[test]
fn cursor() {
    let layout = editor("one\ntwo\nthree", 13, 12, 2);
    assert_eq!(layout.lines, vec!["     two", "     three"]);
    assert_eq!(layout.row, 1);
    assert_eq!(layout.column, 10);
    assert_eq!(layout.top, 1);
    assert_eq!(layout.total, 3);
}
