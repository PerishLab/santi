use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

use crate::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = thinking::State)]
pub enum State {
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = thinking::Reason)]
pub enum Reason {
    Spoke,
    Called,
    Finished,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = thinking::Span)]
pub struct Span {
    pub id: String,
    pub turn: String,
    pub response: Option<String>,
    pub state: State,
    pub summary: Option<String>,
    pub completion_reason: Option<Reason>,
    pub error: Option<String>,
    pub created: Timestamp,
    pub updated: Timestamp,
    pub finished: Option<Timestamp>,
    pub round: Option<Round>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = thinking::Round)]
pub struct Round {
    pub code: Option<u16>,
    pub millis: Option<u64>,
    pub headers: BTreeMap<String, String>,
    pub status: Option<String>,
    pub incomplete: Option<String>,
    pub model: Option<String>,
    pub usage: Option<Usage>,
    pub text: usize,
    pub calls: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = thinking::Usage)]
pub struct Usage {
    pub input: Option<u64>,
    pub cached: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "beat", rename_all = "snake_case")]
#[schema(as = thinking::Beat)]
pub enum Beat {
    Created { thinking: Span },
    Updated { thinking: Span },
    Completed { thinking: Span },
}
