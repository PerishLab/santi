use super::{Content, Descriptor};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Deserialize, Serialize, IntoParams)]
pub struct Query {
    pub reference: Option<String>,
    pub blob: Option<String>,
    #[serde(default)]
    pub effective: bool,
    pub operation: Option<String>,
    pub context: Option<String>,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub offset: usize,
    #[serde(default = "limit")]
    pub limit: usize,
}

fn limit() -> usize {
    20
}

impl Default for Query {
    fn default() -> Self {
        Self {
            reference: None,
            blob: None,
            effective: false,
            operation: None,
            context: None,
            available: false,
            offset: 0,
            limit: limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = material::Page)]
pub struct Page {
    pub items: Vec<Descriptor>,
    pub next: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
#[schema(as = material::View)]
pub enum View {
    State(super::State),
    Change(super::Change),
    Context(super::Context),
    Contexts { contexts: Vec<super::Context> },
    Page(Page),
    Material(Descriptor),
    Blob(Content),
}
