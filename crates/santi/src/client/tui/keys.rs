use std::io::stdout;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;
use std::time::Duration;

use ratatui::crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use ratatui::crossterm::{
    event::{DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture},
    execute,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

pub(super) enum Stroke {
    Enter,
    Newline,
    Paste(String),
    Erase,
    Delete,
    Left,
    Right,
    Home,
    End,
    WordLeft,
    WordRight,
    Up,
    Down,
    ScrollUp,
    ScrollDown,
    PageUp,
    PageDown,
    Ahead,
    Behind,
    Shed,
    Bottom,
    Verbose,
    Copy,
    Press(u16, u16),
    Drag(u16, u16),
    Release,
    Interrupt,
    Quit,
    Typed(char),
    Redraw,
}

pub(super) struct Listener {
    receiver: UnboundedReceiver<Stroke>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    pub(super) async fn recv(&mut self) -> Option<Stroke> {
        self.receiver.recv().await
    }

    pub(super) fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn listen() -> Listener {
    let (sender, receiver) = unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let halt = stop.clone();
    let thread = std::thread::spawn(move || {
        let mut output = stdout();
        let _ = execute!(output, EnableBracketedPaste, EnableMouseCapture);
        pump(&sender, &halt);
        let _ = execute!(output, DisableMouseCapture, DisableBracketedPaste);
    });
    Listener {
        receiver,
        stop,
        thread: Some(thread),
    }
}

fn pump(sender: &UnboundedSender<Stroke>, stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) && !sender.is_closed() {
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
        Event::Paste(text) => Some(Stroke::Paste(text)),
        Event::Key(key) => pressed(key),
        Event::Mouse(mouse) => rolled(mouse),
        _ => None,
    }
}

fn rolled(mouse: MouseEvent) -> Option<Stroke> {
    match mouse.kind {
        MouseEventKind::ScrollUp => Some(Stroke::ScrollUp),
        MouseEventKind::ScrollDown => Some(Stroke::ScrollDown),
        MouseEventKind::Down(MouseButton::Left) => Some(Stroke::Press(mouse.column, mouse.row)),
        MouseEventKind::Drag(MouseButton::Left) => Some(Stroke::Drag(mouse.column, mouse.row)),
        MouseEventKind::Up(MouseButton::Left) => Some(Stroke::Release),
        _ => None,
    }
}

pub(super) fn pressed(key: KeyEvent) -> Option<Stroke> {
    if key.kind == KeyEventKind::Release {
        return None;
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Enter if shift => Some(Stroke::Newline),
        KeyCode::Enter => Some(Stroke::Enter),
        KeyCode::Char('j' | '\n') if control => Some(Stroke::Newline),
        KeyCode::Backspace if control || alt => Some(Stroke::Shed),
        KeyCode::Backspace => Some(Stroke::Erase),
        KeyCode::Delete => Some(Stroke::Delete),
        KeyCode::Left if alt => Some(Stroke::WordLeft),
        KeyCode::Right if alt => Some(Stroke::WordRight),
        KeyCode::Left => Some(Stroke::Left),
        KeyCode::Right => Some(Stroke::Right),
        KeyCode::Home if control => Some(Stroke::Bottom),
        KeyCode::End if control => Some(Stroke::Bottom),
        KeyCode::Home => Some(Stroke::Home),
        KeyCode::End => Some(Stroke::End),
        KeyCode::Char('u') if control => Some(Stroke::Ahead),
        KeyCode::Char('k') if control => Some(Stroke::Behind),
        KeyCode::Char('w') if control => Some(Stroke::Shed),
        KeyCode::Char('a') if control => Some(Stroke::Home),
        KeyCode::Char('e') if control => Some(Stroke::End),
        KeyCode::Char('t') if control => Some(Stroke::Verbose),
        KeyCode::Char('y') if control => Some(Stroke::Copy),
        KeyCode::Esc => Some(Stroke::Interrupt),
        KeyCode::Up if control => Some(Stroke::ScrollUp),
        KeyCode::Down if control => Some(Stroke::ScrollDown),
        KeyCode::Up => Some(Stroke::Up),
        KeyCode::Down => Some(Stroke::Down),
        KeyCode::PageUp => Some(Stroke::PageUp),
        KeyCode::PageDown => Some(Stroke::PageDown),
        KeyCode::Char('c' | 'd') if control => Some(Stroke::Quit),
        KeyCode::Char(character) if !control => Some(Stroke::Typed(character)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Stroke, pressed};
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
}
