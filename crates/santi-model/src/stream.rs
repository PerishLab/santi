use serde::{Deserialize, Serialize};

use crate::Timestamp;
use utoipa::ToSchema;

use santi_error::{Incident, Transition};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = stream::Event)]
pub struct Event {
    pub id: String,
    pub strand: String,
    pub created: Timestamp,
    pub payload: Payload,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
#[schema(as = stream::Payload)]
pub enum Payload {
    Open,
    Message(crate::message::Beat),
    Tool(crate::tool::Beat),
    Thinking(crate::thinking::Beat),
    Turn(crate::turn::Beat),
    #[serde(rename = "material")]
    Render(crate::render::Beat),
    Transition {
        transition: Box<Transition>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = stream::Snapshot)]
pub struct Snapshot {
    pub strand: crate::strand::Strand,
    pub messages: Vec<crate::message::Placed>,
    pub events: Vec<crate::message::Event>,
    pub turns: Vec<crate::turn::Turn>,
    pub thinking: Vec<crate::thinking::Span>,
    #[serde(default)]
    pub contexts: Vec<crate::material::Context>,
    pub calls: Vec<crate::tool::Call>,
    pub results: Vec<crate::tool::Reply>,
    pub compacts: Vec<crate::compact::Compact>,
    pub effects: Vec<crate::effect::Effect>,
    pub errors: Vec<Incident>,
}

pub const RECORDS: usize = 16;
pub const DETAIL: usize = 2_048;
pub const RESPONSE: usize = 36_864;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = stream::ExecutionOrder)]
pub enum Order {
    #[serde(rename = "newest_first")]
    Newest,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = stream::ExecutionTail)]
pub struct Tail {
    pub order: Order,
    #[serde(rename = "record_limit")]
    pub limit: usize,
    #[serde(rename = "detail_limit_bytes")]
    pub detail: usize,
    #[serde(rename = "older_omitted")]
    pub omitted: bool,
    pub records: Vec<Record>,
}

impl Tail {
    pub fn newest(records: Vec<Record>, omitted: bool) -> Self {
        debug_assert!(records.len() <= RECORDS);
        Self {
            order: Order::Newest,
            limit: RECORDS,
            detail: DETAIL,
            omitted,
            records,
        }
    }

    pub fn bounded(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("execution tail serialization failed: {error}"))?
            .len();
        if bytes > RESPONSE {
            return Err(format!("execution tail response exceeded {RESPONSE} bytes"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = stream::ExecutionRecord)]
pub struct Record {
    pub sequence: i64,
    pub created: Timestamp,
    pub kind: crate::strand::Target,
    pub detail: String,
    #[serde(rename = "detail_bytes")]
    pub bytes: u64,
    #[serde(rename = "detail_truncated")]
    pub truncated: bool,
}

impl Record {
    pub fn project<T: Serialize>(
        sequence: i64,
        created: Timestamp,
        kind: crate::strand::Target,
        record: &T,
    ) -> Result<Self, serde_json::Error> {
        let serialized = serde_json::to_string(record)?;
        let bytes = u64::try_from(serialized.len()).unwrap_or(u64::MAX);
        let detail = clip(&serialized);
        let truncated = detail.len() < serialized.len();
        Ok(Self {
            sequence,
            created,
            kind,
            detail,
            bytes,
            truncated,
        })
    }
}

fn clip(serialized: &str) -> String {
    let mut raw = 0;
    let mut wire = 0;
    let mut end = 0;
    for (offset, character) in serialized.char_indices() {
        let size = character.len_utf8();
        let transferred = match character {
            '"' | '\\' => 2,
            '\u{0008}' | '\u{000c}' | '\n' | '\r' | '\t' => 2,
            character if character <= '\u{001f}' => 6,
            character => character.len_utf8(),
        };
        if raw + size > DETAIL || wire + transferred > DETAIL {
            break;
        }
        raw += size;
        wire += transferred;
        end = offset + size;
    }
    serialized[..end].to_string()
}
