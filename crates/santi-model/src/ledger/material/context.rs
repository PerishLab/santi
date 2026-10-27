use super::Provenance;
use crate::Timestamp;
use crate::environ::Scope;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = material::Component)]
pub struct Component {
    pub reference: String,
    pub revision: String,
    pub origins: Vec<Origin>,
    pub entry: Vec<String>,
    pub bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = material::Context)]
pub struct Context {
    pub id: String,
    pub turn: String,
    pub round: usize,
    pub thinking: Option<String>,
    pub created: Timestamp,
    pub digest: String,
    pub materials: Vec<Component>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = material::Origin)]
pub struct Origin {
    pub scope: Scope,
    pub provenance: Provenance,
}
