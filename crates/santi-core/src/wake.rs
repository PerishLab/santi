use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const ROUNDS: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Active,
    Expired,
    Revoked,
    Silent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct Lease {
    pub soul: String,
    pub generation: u64,
    pub state: State,
    pub rounds_remaining: u8,
    pub cadence_millis: u64,
    pub next_at: Option<String>,
    pub last_wake_at: Option<String>,
    pub updated: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallerAction {
    Enable,
    Disable,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
pub struct CallerRequest {
    pub action: CallerAction,
}

impl Lease {
    pub(crate) fn revoked(soul: &str, cadence_millis: u64) -> Self {
        Self {
            soul: soul.to_string(),
            generation: 0,
            state: State::Revoked,
            rounds_remaining: 0,
            cadence_millis,
            next_at: None,
            last_wake_at: None,
            updated: None,
        }
    }
}
