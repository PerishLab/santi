use async_trait::async_trait;
use futures_core::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{pin::Pin, sync::Arc};

pub use santi_model::thinking::Usage;

#[derive(Debug, Clone)]
pub enum Item {
    Boundary,
    Continuation(santi_model::thinking::Continuation),
    Message {
        role: String,
        content: String,
    },
    Call {
        call: String,
        name: String,
        raw: String,
    },
    Output {
        call: String,
        output: String,
    },
}

#[derive(Debug, Clone)]
pub struct Request {
    pub model: String,
    pub instructions: Option<String>,
    pub input: Vec<Item>,
    pub tools: Option<Vec<Tool>>,
    pub previous: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    Function(Function),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    pub response: String,
    pub mark: Option<String>,
    pub item: Value,
    pub call: String,
    pub name: String,
    pub raw: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trace {
    Chunk { bytes: usize },
    Raw { kind: String, mapped: Vec<String> },
    Retried { attempt: u32, detail: String },
    Received(Receipt),
    Reported(Report),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Receipt {
    pub status: u16,
    pub elapsed: u64,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub status: Option<String>,
    pub incomplete: Option<String>,
    pub model: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone)]
pub struct Metadata {
    pub provider: Arc<str>,
    pub model: String,
    pub budget: Option<Cap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cap {
    pub bytes: usize,
    pub maintenance: usize,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Traced(Trace),
    Started { response: Option<String> },
    Working { response: Option<String> },
    Thinking(String),
    Retained(santi_model::thinking::Continuation),
    Thought(String),
    Text(String),
    Called(Call),
    Completed { response: Option<String> },
    Failed(String),
}

pub type Streaming = Pin<Box<dyn Stream<Item = Result<Event, String>> + Send + 'static>>;

#[async_trait]
pub trait Provider: Send + Sync {
    fn metadata(&self) -> Metadata;

    fn prepare(&self, mut request: Request) -> Result<Request, String> {
        request
            .input
            .retain(|item| !matches!(item, Item::Continuation(_) | Item::Boundary));
        Ok(request)
    }

    fn validate(&self, _: &Request) -> Result<(), String> {
        Ok(())
    }

    fn estimate(&self, _: &Request) -> Option<santi_model::budget::Estimate> {
        None
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String>;
}
