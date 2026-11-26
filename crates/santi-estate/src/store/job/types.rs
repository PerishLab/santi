use santi_model::job;
use std::collections::BTreeMap;

#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Environment(pub BTreeMap<String, String>);

impl std::fmt::Debug for Environment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Environment(<private>)")
    }
}

#[derive(Clone, Copy)]
pub struct Capability<'a> {
    pub digest: &'a str,
    pub expires: i64,
    pub soul: &'a str,
    pub strand: &'a str,
    pub turn: &'a str,
    pub call: &'a str,
    pub effect: &'a str,
    pub created: &'a str,
}

#[derive(Clone, Copy)]
pub struct Job<'a> {
    pub environment: Option<&'a Environment>,
    pub tag: &'a str,
    pub description: &'a str,
    pub command: &'a str,
    pub cwd: Option<&'a str>,
    pub timeout: u64,
    pub limit: u64,
    pub remind: Option<u64>,
    pub notify: bool,
    pub digest: &'a str,
    pub generation: &'a str,
    pub supervisor: &'a str,
    pub created: &'a str,
}

pub struct Transition<'a> {
    pub base: Option<&'a job::State>,
    pub state: job::State,
    pub reason: Option<&'a str>,
    pub exit: Option<i32>,
    pub occurred: &'a str,
    pub started: Option<i64>,
    pub next: Option<&'a str>,
}

impl<'a> Transition<'a> {
    pub fn new(state: job::State, occurred: &'a str) -> Self {
        Self {
            base: None,
            state,
            reason: None,
            exit: None,
            occurred,
            started: None,
            next: None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Attention<'a> {
    pub job: &'a str,
    pub base: u64,
    pub at: &'a str,
    pub runtime: bool,
    pub output: bool,
    pub reminded: bool,
    pub tick: u64,
    pub next: Option<&'a str>,
    pub terminal: bool,
}

#[derive(Debug, Clone)]
pub struct Record {
    pub environment: Option<Environment>,
    pub job: job::Job,
    pub generation: String,
    pub supervisor: String,
    pub started: Option<i64>,
    pub attention: u64,
    pub runtime: bool,
    pub output: bool,
    pub tick: u64,
    pub notify: bool,
    pub notified: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Prepared {
    New(Record),
    Existing(Record),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expired {
    pub id: String,
    pub key: String,
}
