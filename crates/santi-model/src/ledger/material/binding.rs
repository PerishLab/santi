use super::{Package, Provenance};
use crate::{Timestamp, environ::Scope};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = material::Command)]
pub struct Command {
    pub reference: String,
    #[serde(default)]
    pub operation: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[schema(as = material::Binding)]
pub struct Binding {
    pub id: String,
    pub scope: Scope,
    pub owner: String,
    pub reference: String,
    pub revision: String,
    pub provenance: Provenance,
    pub operation: String,
    pub updated: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(as = material::Selection)]
pub struct Selection {
    pub scope: Scope,
    pub owner: String,
    pub bindings: Vec<Binding>,
    pub packages: Vec<Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = material::State)]
pub struct State {
    pub scope: Scope,
    pub owner: String,
    pub bindings: Vec<Binding>,
    pub effective: Vec<Binding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = material::Change)]
pub struct Change {
    pub id: String,
    pub scope: Scope,
    pub owner: String,
    pub action: String,
    pub reference: String,
    pub previous: Option<String>,
    pub current: Option<String>,
    pub created: Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = material::Applied)]
pub struct Applied {
    pub change: Change,
    pub state: State,
    pub estimate: Option<crate::budget::Estimate>,
    pub boundary: String,
}
