#[path = "../../src/client/tui/keys.rs"]
#[allow(dead_code)]
mod inner;

use inner::{Stroke, pressed};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode, modifiers: KeyModifiers) -> Option<Stroke> {
    pressed(KeyEvent::new(code, modifiers))
}

#[test]
fn bound() {
    assert!(matches!(
        key(KeyCode::End, KeyModifiers::CONTROL),
        Some(Stroke::Bottom)
    ));
    assert!(matches!(
        key(KeyCode::End, KeyModifiers::NONE),
        Some(Stroke::End)
    ));
    assert!(matches!(
        key(KeyCode::PageDown, KeyModifiers::NONE),
        Some(Stroke::PageDown)
    ));
    assert!(matches!(
        key(KeyCode::Enter, KeyModifiers::SHIFT),
        Some(Stroke::Newline)
    ));
    assert!(matches!(
        key(KeyCode::Enter, KeyModifiers::NONE),
        Some(Stroke::Enter)
    ));
    assert!(matches!(
        key(KeyCode::Char('t'), KeyModifiers::CONTROL),
        Some(Stroke::Verbose)
    ));
    assert!(matches!(
        key(KeyCode::Esc, KeyModifiers::NONE),
        Some(Stroke::Interrupt)
    ));
}
