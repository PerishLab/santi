use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = job::State)]
pub enum State {
    Submitting,
    Accepted,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    #[serde(rename = "timed_out")]
    Timeout,
    Cancelled,
    Unknown,
}

impl State {
    pub fn terminal(&self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Timeout | Self::Cancelled | Self::Unknown
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = job::Origin)]
pub struct Origin<T = String> {
    #[schema(value_type = String)]
    pub soul: T,
    #[schema(value_type = String)]
    pub strand: T,
    #[schema(value_type = String)]
    pub turn: T,
    #[schema(value_type = String)]
    pub call: T,
    #[schema(value_type = String)]
    pub effect: T,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = job::Job)]
pub struct Job {
    pub id: String,
    pub origin: Origin,
    pub description: String,
    pub command: String,
    pub cwd: Option<String>,
    #[serde(rename = "timeout_seconds")]
    pub timeout: u64,
    #[serde(rename = "output_limit_bytes")]
    pub limit: u64,
    #[serde(rename = "remind_every_seconds")]
    pub remind: Option<u64>,
    pub state: State,
    pub reason: Option<String>,
    #[serde(rename = "exit_code")]
    pub exit: Option<i32>,
    pub created: Timestamp,
    pub updated: Timestamp,
    pub accepted: Option<Timestamp>,
    pub started: Option<Timestamp>,
    #[serde(rename = "last_reminded")]
    pub last: Option<Timestamp>,
    #[serde(rename = "next_reminder")]
    pub next: Option<Timestamp>,
    pub finished: Option<Timestamp>,
    pub acknowledged: Option<Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = job::Accepted)]
pub struct Accepted {
    pub job: Job,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = job::Stream)]
pub enum Stream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = job::Log)]
pub struct Log {
    pub job: String,
    pub stream: Stream,
    pub cursor: String,
    pub next: String,
    pub eof: bool,
    pub data: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = job::QueryRequest)]
pub struct Request {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub strand: Option<String>,
    #[serde(default, rename = "all_strands")]
    pub all: bool,
    #[serde(default)]
    pub state: Vec<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub count: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(untagged)]
#[schema(as = job::Query)]
pub enum Query {
    Rows(Rows),
    Count(Count),
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = job::QueryRows)]
pub struct Rows {
    pub rows: Vec<Job>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = job::QueryCount)]
pub struct Count {
    pub count: usize,
}

#[derive(Debug, Clone)]
pub enum Fault {
    Invalid(String),
    Failed(String),
}

impl Fault {
    pub fn message(&self) -> &str {
        match self {
            Self::Invalid(message) | Self::Failed(message) => message,
        }
    }
}
