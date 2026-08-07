use std::time::Duration;

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub(super) enum Stroke {
    Enter,
    Erase,
    Quit,
    Up,
    Down,
    Typed(char),
    Redraw,
}

pub(super) fn listen() -> UnboundedReceiver<Stroke> {
    let (sender, receiver) = unbounded_channel();
    std::thread::spawn(move || pump(&sender));
    receiver
}

fn pump(sender: &UnboundedSender<Stroke>) {
    while !sender.is_closed() {
        if !event::poll(Duration::from_millis(200)).unwrap_or(false) {
            continue;
        }
        let Ok(event) = event::read() else {
            return;
        };
        let Some(stroke) = translate(event) else {
            continue;
        };
        if sender.send(stroke).is_err() {
            return;
        }
    }
}

fn translate(event: Event) -> Option<Stroke> {
    match event {
        Event::Resize(_, _) => Some(Stroke::Redraw),
        Event::Key(key) => pressed(key),
        _ => None,
    }
}

fn pressed(key: KeyEvent) -> Option<Stroke> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Enter => Some(Stroke::Enter),
        KeyCode::Backspace => Some(Stroke::Erase),
        KeyCode::Char('c' | 'd') if control => Some(Stroke::Quit),
        KeyCode::PageUp => Some(Stroke::Up),
        KeyCode::PageDown => Some(Stroke::Down),
        KeyCode::Char(character) if !control => Some(Stroke::Typed(character)),
        _ => None,
    }
}
