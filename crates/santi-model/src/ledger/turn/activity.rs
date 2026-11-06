use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = turn::activity::Phase)]
pub enum Phase {
    Preparing,
    Requesting,
    Responding,
    Generating,
    Calling,
    Invoking,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = turn::activity::Current)]
pub struct Current {
    pub turn: String,
    pub round: usize,
    pub phase: Phase,
    pub call: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = turn::activity::Snapshot)]
pub struct Snapshot {
    pub epoch: String,
    pub revision: u64,
    pub strand: String,
    pub current: Option<Current>,
}
