use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[schema(as = receipt::reply::Binding)]
pub struct Binding {
    pub adaptor: String,
    pub context: Value,
    pub delay: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = receipt::reply::Owner)]
pub enum Owner {
    Runtime,
    Soul,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = receipt::reply::State)]
pub enum State {
    Queued,
    Cancelled,
    Dispatching,
    Accepted,
    Rejected,
    Unsent,
    Unknown,
    Observed,
    NotApplied,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[schema(as = receipt::reply::Attempt)]
pub struct Attempt {
    pub operation: String,
    pub uuid: String,
    pub owner: Owner,
    pub body: Value,
    pub state: State,
    pub target: Option<String>,
    pub result: Option<Value>,
    pub resolution: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[schema(as = receipt::reply::Window)]
pub struct Window {
    pub binding: Binding,
    pub owner: Owner,
    pub message: Option<String>,
    pub attempts: Vec<Attempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = receipt::reply::Status)]
pub struct Status {
    pub inbox: String,
    pub strand: String,
    pub accepted: String,
    pub window: Window,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = receipt::reply::Request)]
pub struct Request {
    pub strand: String,
    pub operation: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = receipt::reply::Resolution)]
pub struct Resolution {
    pub operation: String,
    pub applied: bool,
    pub message: Option<String>,
    pub evidence: String,
}
