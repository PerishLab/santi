use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    #[default]
    Native,
    Skill,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub root: String,
    #[serde(default)]
    pub format: Format,
}

pub type Sources = BTreeMap<String, Source>;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[schema(as = material::Provenance)]
pub struct Provenance {
    pub source: String,
    pub format: String,
    pub version: Option<String>,
    pub keeper: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[schema(as = material::Blob)]
pub struct Blob {
    pub name: String,
    pub mime: String,
    pub bytes: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[schema(as = material::Descriptor)]
pub struct Descriptor {
    pub reference: String,
    pub name: String,
    pub description: String,
    pub revision: String,
    pub entry: Vec<String>,
    pub blobs: Vec<Blob>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[schema(as = material::Content)]
pub struct Content {
    pub name: String,
    pub mime: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[schema(as = material::Package)]
pub struct Package {
    pub material: Descriptor,
    pub contents: Vec<Content>,
}
