use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::Timestamp;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
#[schema(as = render::Kind)]
pub enum Kind {
    SystemPrompt,
    Skill,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = render::Request)]
pub struct Request {
    pub kind: Kind,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = render::View)]
pub struct View {
    pub strand: String,
    pub kind: Kind,
    pub content_type: String,
    pub text: String,
    pub updated: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = render::Updated)]
pub struct Updated {
    pub strand: String,
    pub kind: Kind,
    pub updated: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "beat", rename_all = "snake_case")]
#[schema(as = render::Beat)]
pub enum Beat {
    Updated {
        #[serde(rename = "material")]
        view: Updated,
    },
}
