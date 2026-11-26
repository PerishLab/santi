use axum::http::HeaderMap;
use serde_json::Value;

pub mod cron;
pub mod feishu;
pub mod github;

use feishu::Feishu;
use github::Github;

#[derive(Debug, Clone)]
pub struct Event {
    pub text: String,
    pub label: String,
    pub metadata: Option<Value>,
    pub delivery: Option<String>,
    pub scoped: bool,
    pub authored: bool,
}

#[derive(Debug, Clone)]
pub enum Outcome {
    Event(Event),
    Reply(Value),
}

#[derive(Debug)]
pub enum Error {
    Unauthorized(String),
    Invalid(String),
    Unavailable(String),
}

pub trait Adaptor: Send + Sync {
    fn verify(&self, headers: &HeaderMap, raw: &[u8], secret: &str) -> Result<(), Error>;

    fn normalize(
        &self,
        headers: &HeaderMap,
        raw: &[u8],
        secret: &str,
        name: &str,
    ) -> Result<Outcome, Error>;
}

pub(crate) fn resolve(adaptor: &str, account: Option<&str>) -> Option<Box<dyn Adaptor>> {
    match adaptor {
        "github" => {
            let held = crate::runtime::held();
            Some(Box::new(
                Github::configured(held.github.login.as_deref(), held.github.allow.as_deref())
                    .addressed(account),
            ))
        }
        "feishu" => {
            let held = crate::runtime::held();
            Some(Box::new(Feishu::configured(
                held.feishu.secret.as_deref(),
                held.feishu.allow.as_deref(),
            )))
        }
        _ => None,
    }
}

fn allowed(sender: &str, allow: Option<&str>) -> bool {
    match allow.map(str::trim) {
        None | Some("") => true,
        Some(list) => list
            .split(',')
            .any(|entry| entry.trim().eq_ignore_ascii_case(sender)),
    }
}

fn field(value: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = value;
    for key in path {
        cursor = cursor.get(key)?;
    }
    cursor.as_str().map(str::to_string)
}
