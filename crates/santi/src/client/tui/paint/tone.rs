use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    You,
    Soul,
    Body,
    Notice,
    Activity,
}

impl Tone {
    pub(super) fn style(self) -> Style {
        match self {
            Tone::You => Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            Tone::Soul => Style::new().fg(Color::Green).add_modifier(Modifier::BOLD),
            Tone::Body => Style::new(),
            Tone::Notice => Style::new().fg(Color::Yellow),
            Tone::Activity => Style::new().add_modifier(Modifier::DIM),
        }
    }
}
