use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub mod selection;
pub mod validate;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Record {
    pub id: i64,
    #[serde(flatten)]
    pub definition: Definition,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Amendment {
    pub expected: Record,
    pub replacement: Definition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    ChatCompletions,
    #[serde(rename = "openai_responses")]
    OpenAiResponses,
    #[serde(rename = "gemini_generate_content")]
    Gemini,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Definition {
    Provider {
        name: String,
    },
    Family {
        name: String,
    },
    Scenario {
        name: String,
        criterion: String,
    },
    Access {
        name: String,
        provider: i64,
        protocol: Protocol,
        endpoint: String,
        credential: String,
    },
    Alias {
        name: String,
        access: i64,
    },
    Offering {
        access: i64,
        family: i64,
        model: String,
    },
    Candidate {
        scenario: i64,
        offering: i64,
        evidence: String,
    },
    Preference {
        candidate: i64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Provider,
    Family,
    Scenario,
    Access,
    Alias,
    Offering,
    Candidate,
    Preference,
}

impl Definition {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Provider { .. } => Kind::Provider,
            Self::Family { .. } => Kind::Family,
            Self::Scenario { .. } => Kind::Scenario,
            Self::Access { .. } => Kind::Access,
            Self::Alias { .. } => Kind::Alias,
            Self::Offering { .. } => Kind::Offering,
            Self::Candidate { .. } => Kind::Candidate,
            Self::Preference { .. } => Kind::Preference,
        }
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Provider { name }
            | Self::Family { name }
            | Self::Scenario { name, .. }
            | Self::Access { name, .. }
            | Self::Alias { name, .. } => Some(name),
            _ => None,
        }
    }

    pub fn references(&self) -> Vec<(Kind, i64)> {
        match self {
            Self::Access { provider, .. } => vec![(Kind::Provider, *provider)],
            Self::Alias { access, .. } => vec![(Kind::Access, *access)],
            Self::Offering { access, family, .. } => {
                vec![(Kind::Access, *access), (Kind::Family, *family)]
            }
            Self::Candidate {
                scenario, offering, ..
            } => vec![(Kind::Scenario, *scenario), (Kind::Offering, *offering)],
            Self::Preference { candidate } => vec![(Kind::Candidate, *candidate)],
            _ => vec![],
        }
    }
}

impl Kind {
    pub const ALL: [Self; 8] = [
        Self::Provider,
        Self::Family,
        Self::Scenario,
        Self::Access,
        Self::Alias,
        Self::Offering,
        Self::Candidate,
        Self::Preference,
    ];

    pub fn unit(self) -> &'static str {
        match self {
            Self::Provider => "Provider",
            Self::Family => "Family",
            Self::Scenario => "Scenario",
            Self::Access => "Access",
            Self::Alias => "Alias",
            Self::Offering => "Offering",
            Self::Candidate => "Candidate",
            Self::Preference => "Preference",
        }
    }
}
