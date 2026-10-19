use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = alarm::Alarm)]
pub struct Alarm {
    pub id: String,
    pub name: String,
    pub soul: String,
    pub strand: String,
    pub text: String,
    pub cron: String,
    pub generation: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = alarm::Draft)]
pub struct Draft {
    pub soul: String,
    pub strand: String,
    pub name: String,
    pub text: String,
    pub cron: String,
    pub generation: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = alarm::Projection)]
pub struct Projection {
    pub endpoint: String,
    pub timezone: String,
    pub applied: bool,
    pub observed: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = alarm::View)]
pub struct View {
    pub alarms: Vec<Alarm>,
    pub projection: Projection,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
#[schema(as = alarm::Occurrence)]
pub struct Occurrence {
    pub generation: i64,
    pub minute: i64,
}
