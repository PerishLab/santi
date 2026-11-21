use super::*;
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct Label {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
pub struct Route {
    pub provider: Label,
    pub access: Label,
    pub family: Option<Label>,
    pub scenario: Option<Label>,
    pub offering: Option<i64>,
    pub candidate: Option<i64>,
    pub protocol: String,
    pub endpoint: String,
    pub custody: String,
    pub identity: String,
    pub policy: String,
    pub replay: bool,
    pub thinking: Option<String>,
}
