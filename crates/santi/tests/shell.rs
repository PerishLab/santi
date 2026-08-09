#![allow(clippy::duplicate_mod)]

mod config {
    pub use santi::config::{Resume, executable, resume};

    pub fn env(name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    pub fn shelter() -> std::path::PathBuf {
        std::path::PathBuf::from("/nonexistent")
    }
}

mod watch {
    pub fn snippet(text: &str, limit: usize) -> String {
        text.chars().take(limit).collect()
    }
}

#[path = "shell/budget.rs"]
mod budget;
#[path = "shell/clip.rs"]
mod clip;
#[path = "shell/edge.rs"]
mod edge;
#[path = "shell/keys.rs"]
mod keys;
#[path = "shell/layout.rs"]
mod layout;
#[path = "shell/reload.rs"]
mod reload;
#[path = "shell/seat.rs"]
mod seat;
#[path = "shell/tui.rs"]
mod tui;
