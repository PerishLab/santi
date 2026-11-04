use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use santi_error::Incident;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Estimate)]
pub struct Estimate {
    pub estimator: String,
    pub items: i64,
    pub input: i64,
    pub instructions: i64,
    pub tools: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Cap, description = "bytes is normal capacity and fallback request ceiling; maintenance is effective request ceiling; source names config origin.")]
pub struct Cap {
    pub bytes: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<i64>,
    pub source: String,
}

impl Cap {
    pub fn ceiling(&self) -> i64 {
        self.maintenance.unwrap_or(self.bytes)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Request, description = "Request-size projection for this audit, not a sent record; basis names the tool projection, pending says inbox bytes are included, estimate is the request estimate, ceiling is the effective request ceiling or null without budget. Request and pressure are separate reads, not an atomic snapshot, so cross-projection sums need not reconcile.")]
pub struct Request {
    pub basis: String,
    pub pending: bool,
    pub estimate: Estimate,
    pub ceiling: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Pressure, description = "Ordinary-tools pending-pressure projection, measured independently of Request; basis names the tool projection, pending says inbox bytes are included, active_bytes is gated by active_ceiling_bytes, absent without budget. Request and pressure are separate reads, not an atomic snapshot, so cross-projection sums need not reconcile.")]
pub struct Pressure {
    pub basis: String,
    pub pending: bool,
    #[serde(rename = "active_bytes")]
    pub active: i64,
    #[serde(rename = "active_ceiling_bytes")]
    pub ceiling: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = budget::Snapshot)]
pub struct Snapshot {
    pub strand: String,
    pub request: Request,
    pub pressure: Option<Pressure>,
    #[serde(default)]
    pub materials: Vec<crate::material::Component>,
    pub budget: Option<Cap>,
    pub execution: Option<Execution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slots: Option<Slots>,
    pub usage: Option<Usage>,
    pub incident: Option<Incident>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Slot)]
pub struct Slot {
    pub compact: String,
    pub bytes: i64,
    pub from: i64,
    pub to: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Slots, description = "Slot occupancy and settled-summary allowance, not a live context estimate; count bounds held slots, held lists spans, free is count minus held entries, settled_bytes is the held summary byte sum, settled_ceiling_bytes bounds settled_bytes.")]
pub struct Slots {
    pub count: i64,
    pub held: Vec<Slot>,
    pub free: i64,
    #[serde(rename = "settled_bytes")]
    pub settled: i64,
    #[serde(rename = "settled_ceiling_bytes")]
    pub allowance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Execution)]
pub struct Execution {
    #[serde(default = "timeout")]
    #[schema(default = 60, minimum = 1, maximum = 3600)]
    pub timeout: u64,
    pub profile: String,
    pub rounds: usize,
    pub calls: usize,
    pub output: usize,
    pub shell: usize,
    #[serde(default = "lined")]
    pub lines: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_after_calls: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_cwd: Option<String>,
}

pub fn timeout() -> u64 {
    60
}

impl Execution {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=3600).contains(&self.timeout) {
            return Err("foreground shell timeout must be between 1 and 3600 seconds; use Jobs for longer work".to_string());
        }
        if self.profile.trim().is_empty() {
            return Err("execution budget profile must not be empty".to_string());
        }
        if self.rounds == 0 {
            return Err("execution budget rounds must be positive".to_string());
        }
        if self.calls == 0 {
            return Err("execution budget calls must be positive".to_string());
        }
        if self.output == 0 {
            return Err("execution budget output must be positive".to_string());
        }
        if self.shell == 0 {
            return Err("execution budget shell must be positive".to_string());
        }
        if self.shell > self.output {
            return Err("execution budget shell must not exceed output".to_string());
        }
        if self.lines == 0 {
            return Err("execution budget lines must be positive".to_string());
        }
        if self.feedback_after_calls == Some(0) {
            return Err("execution feedback call threshold must be positive".to_string());
        }
        if self
            .feedback_command
            .as_ref()
            .is_some_and(|command| command.trim().is_empty())
        {
            return Err("execution feedback command must not be empty".to_string());
        }
        if self.feedback_command.is_some() && self.feedback_after_calls.is_none() {
            return Err("execution feedback command requires a call threshold".to_string());
        }
        if self.feedback_cwd.is_some() && self.feedback_command.is_none() {
            return Err("execution feedback cwd requires a caller-owned command".to_string());
        }
        if self
            .feedback_cwd
            .as_ref()
            .is_some_and(|cwd| cwd.trim().is_empty())
        {
            return Err("execution feedback cwd must not be empty".to_string());
        }
        Ok(())
    }
}

fn lined() -> usize {
    256
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Usage)]
pub struct Usage {
    pub calls: usize,
    pub output: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum Error {
    Context,
    Execution,
    Inbox,
}

impl santi_error::Ruled for Error {
    fn descriptor(&self) -> santi_error::Descriptor {
        use santi_error::{Category, Exposure, Retry, Severity};
        match self {
            Self::Context => santi_error::Descriptor {
                code: "context.budget.exceeded",
                category: Category::Exhausted,
                severity: Severity::Error,
                retry: Retry::Resolved,
                exposure: Exposure::CALLER_AND_OPERATOR,
            },
            Self::Execution => santi_error::Descriptor {
                code: "runtime.execution_budget.exceeded",
                category: Category::Exhausted,
                severity: Severity::Error,
                retry: Retry::Changed,
                exposure: Exposure::CALLER_AND_OPERATOR,
            },
            Self::Inbox => santi_error::Descriptor {
                code: "runtime.inbox.capacity_exceeded",
                category: Category::Exhausted,
                severity: Severity::Error,
                retry: Retry::Later,
                exposure: Exposure::CALLER_AND_OPERATOR,
            },
        }
    }
}
