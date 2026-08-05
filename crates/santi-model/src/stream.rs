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
    Material(crate::material::Beat),
    Transition { transition: Box<Transition> },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = stream::Snapshot)]
pub struct Snapshot {
    pub strand: crate::strand::Strand,
    pub messages: Vec<crate::message::Placed>,
    pub events: Vec<crate::message::Event>,
    pub turns: Vec<crate::turn::Turn>,
    pub thinking: Vec<crate::thinking::Span>,
    pub calls: Vec<crate::tool::Call>,
    pub results: Vec<crate::tool::Reply>,
    pub compacts: Vec<crate::compact::Compact>,
    pub effects: Vec<crate::effect::Effect>,
    pub errors: Vec<Incident>,
}

pub const EXECUTION_TAIL_RECORD_LIMIT: usize = 16;
pub const EXECUTION_TAIL_DETAIL_LIMIT_BYTES: usize = 2_048;
pub const EXECUTION_TAIL_RESPONSE_LIMIT_BYTES: usize = 36_864;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schema(as = stream::ExecutionOrder)]
pub enum ExecutionOrder {
    NewestFirst,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = stream::ExecutionTail)]
pub struct ExecutionTail {
    pub order: ExecutionOrder,
    pub record_limit: usize,
    pub detail_limit_bytes: usize,
    pub older_omitted: bool,
    pub records: Vec<ExecutionRecord>,
}

impl ExecutionTail {
    pub fn newest(records: Vec<ExecutionRecord>, older_omitted: bool) -> Self {
        debug_assert!(records.len() <= EXECUTION_TAIL_RECORD_LIMIT);
        Self {
            order: ExecutionOrder::NewestFirst,
            record_limit: EXECUTION_TAIL_RECORD_LIMIT,
            detail_limit_bytes: EXECUTION_TAIL_DETAIL_LIMIT_BYTES,
            older_omitted,
            records,
        }
    }

    pub fn ensure_response_bound(&self) -> Result<(), String> {
        let bytes = serde_json::to_vec(self)
            .map_err(|error| format!("execution tail serialization failed: {error}"))?
            .len();
        if bytes > EXECUTION_TAIL_RESPONSE_LIMIT_BYTES {
            return Err(format!(
                "execution tail response exceeded {EXECUTION_TAIL_RESPONSE_LIMIT_BYTES} bytes"
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(as = stream::ExecutionRecord)]
pub struct ExecutionRecord {
    pub sequence: i64,
    pub created: Timestamp,
    pub kind: crate::strand::Target,
    pub detail: String,
    pub detail_bytes: u64,
    pub detail_truncated: bool,
}

impl ExecutionRecord {
    pub fn project<T: Serialize>(
        sequence: i64,
        created: Timestamp,
        kind: crate::strand::Target,
        record: &T,
    ) -> Result<Self, serde_json::Error> {
        let serialized = serde_json::to_string(record)?;
        let detail_bytes = u64::try_from(serialized.len()).unwrap_or(u64::MAX);
        let detail = bounded_detail(&serialized);
        let detail_truncated = detail.len() < serialized.len();
        Ok(Self {
            sequence,
            created,
            kind,
            detail,
            detail_bytes,
            detail_truncated,
        })
    }
}

fn bounded_detail(serialized: &str) -> String {
    let mut raw_bytes = 0;
    let mut transferred_bytes = 0;
    let mut end = 0;
    for (offset, character) in serialized.char_indices() {
        let raw = character.len_utf8();
        let transferred = match character {
            '"' | '\\' => 2,
            '\u{0008}' | '\u{000c}' | '\n' | '\r' | '\t' => 2,
            character if character <= '\u{001f}' => 6,
            character => character.len_utf8(),
        };
        if raw_bytes + raw > EXECUTION_TAIL_DETAIL_LIMIT_BYTES
            || transferred_bytes + transferred > EXECUTION_TAIL_DETAIL_LIMIT_BYTES
        {
            break;
        }
        raw_bytes += raw;
        transferred_bytes += transferred;
        end = offset + raw;
    }
    serialized[..end].to_string()
}
