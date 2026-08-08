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
#[schema(as = budget::Cap)]
pub struct Cap {
    pub bytes: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = budget::Snapshot)]
pub struct Snapshot {
    pub strand: String,
    pub estimate: Estimate,
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
#[schema(as = budget::Slots)]
pub struct Slots {
    pub ceiling: i64,
    pub count: i64,
    pub held: Vec<Slot>,
    pub free: i64,
    pub active: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[schema(as = budget::Execution)]
pub struct Execution {
    pub profile: String,
    pub rounds: usize,
    pub calls: usize,
    pub output: usize,
    pub shell: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_after_calls: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_cwd: Option<String>,
}

impl Execution {
    pub fn validate(&self) -> Result<(), String> {
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
