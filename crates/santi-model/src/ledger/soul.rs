use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = soul::Soul)]
pub struct Soul {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<Identity>,
    pub created: Timestamp,
    pub updated: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
#[schema(as = soul::Identity)]
pub struct Identity {
    pub name: String,
    #[serde(default)]
    pub channels: std::collections::BTreeMap<String, String>,
}

impl Identity {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("identity name must not be empty".into());
        }
        for (channel, account) in &self.channels {
            if channel.trim().is_empty() || account.trim().is_empty() {
                return Err("identity channel binding must not be empty".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = soul::Draft)]
pub struct Draft {
    #[serde(default)]
    pub memory: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Error {
    Intervention,
}

impl santi_error::Ruled for Error {
    fn descriptor(&self) -> santi_error::Descriptor {
        use santi_error::{Category, Exposure, Retry, Severity};
        match self {
            Self::Intervention => santi_error::Descriptor {
                code: "runtime.soul_memory.intervention_required",
                category: Category::Exhausted,
                severity: Severity::Error,
                retry: Retry::Changed,
                exposure: Exposure::OPERATOR_ONLY,
            },
        }
    }
}
