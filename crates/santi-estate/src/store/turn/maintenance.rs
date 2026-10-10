use super::{Store, read, terminal};
use crate::store::{CompletionDraft, InboxDraft, Limits};
use keel::{
    Tx,
    adapt::{Error, db::Sqlite},
};
use santi_model::{compact, event, tool};

pub enum Attempt {
    Compact(Box<compact::Exec>),
    Rejected(String),
}

pub struct Invocation {
    pub call: tool::Call,
    pub result: String,
    pub attempt: Attempt,
    pub limit: Option<usize>,
}

pub struct Maintenance<'a> {
    pub calls: &'a [Invocation],
    pub limits: Limits,
    pub completion: CompletionDraft<'a>,
    pub inbox: InboxDraft<'a>,
}

pub struct Settlement {
    pub calls: Vec<tool::Call>,
    pub replies: Vec<tool::Reply>,
    pub completed: bool,
    pub event: Option<event::Event>,
}

impl Store {
    pub async fn maintain(&self, draft: Maintenance<'_>) -> Result<Settlement, String> {
        self.core
            .batch(async |tx| {
                terminal::check(tx, draft.completion.turn, draft.inbox.strand).await?;
                let mut calls = Vec::with_capacity(draft.calls.len());
                let mut replies = Vec::with_capacity(draft.calls.len());
                let mut compacted = false;
                for invocation in draft.calls {
                    if invocation.call.turn != draft.completion.turn {
                        return Err(Error::Adapt("maintenance call differs from turn".into()));
                    }
                    let arguments = serde_json::to_string(&invocation.call.arguments)
                        .map_err(|error| Error::Adapt(error.to_string()))?;
                    crate::store::tool::put_call(
                        tx,
                        crate::store::CallDraft {
                            tag: &invocation.call.id,
                            turn: &invocation.call.turn,
                            tool: &invocation.call.tool,
                            arguments: &invocation.call.arguments,
                            created: &invocation.call.created,
                        },
                        &arguments,
                    )
                    .await?;
                    let result = applied(tx, draft.inbox.strand, invocation, draft.limits).await?;
                    compacted |= result.is_ok();
                    let reply = replied(tx, invocation, result).await?;
                    calls.push(invocation.call.clone());
                    replies.push(reply);
                }
                let event = if compacted {
                    terminal::handoff(tx, draft.completion, &draft.inbox).await?
                } else {
                    None
                };
                Ok(Settlement {
                    calls,
                    replies,
                    completed: compacted,
                    event,
                })
            })
            .await
            .map_err(read::error)
    }
}

async fn applied(
    tx: &mut Tx<'_, Sqlite>,
    strand: &str,
    invocation: &Invocation,
    limits: Limits,
) -> Result<Result<compact::Report, String>, Error> {
    let request = match &invocation.attempt {
        Attempt::Compact(request) => {
            if invocation.call.tool != "compact" {
                return Err(Error::Adapt(
                    "maintenance compact differs from call tool".into(),
                ));
            }
            request
        }
        Attempt::Rejected(error) => return Ok(Err(error.clone())),
    };
    if request.dry || request.capsule.is_some() {
        return Ok(Err(
            "maintenance compact accepts summary, range and absorb".into()
        ));
    }
    let bounds = crate::store::timeline::compact::selected(tx, strand, request, limits).await?;
    let (first, last) = match bounds {
        Ok(bounds) => bounds,
        Err(error) => return Ok(Err(error)),
    };
    let tag = santi_model::tag("compact");
    let report = crate::store::timeline::compact::create(
        tx,
        &crate::store::CompactDraft {
            tag: &tag,
            strand,
            first: &first,
            last: &last,
            summary: request.summary.trim(),
            metadata: None,
            expected: None,
            created: &invocation.call.created,
        },
        None,
    )
    .await?;
    Ok(Ok(report))
}

async fn replied(
    tx: &mut Tx<'_, Sqlite>,
    invocation: &Invocation,
    result: Result<compact::Report, String>,
) -> Result<tool::Reply, Error> {
    let (output, error) = match result {
        Ok(report) => (
            Some(crate::store::bounded(
                serde_json::to_value(report).map_err(|error| Error::Adapt(error.to_string()))?,
                invocation.limit,
            )),
            None,
        ),
        Err(error) => (None, Some(crate::store::curbed(error, invocation.limit))),
    };
    let serialized = output
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| Error::Adapt(error.to_string()))?;
    crate::store::tool::put_reply(
        tx,
        crate::store::ReplyDraft {
            tag: &invocation.result,
            call: &invocation.call.id,
            output: output.as_ref(),
            error: error.as_deref(),
            created: &invocation.call.created,
        },
        serialized.as_deref(),
    )
    .await?;
    Ok(tool::Reply {
        id: invocation.result.clone(),
        call: invocation.call.id.clone(),
        output,
        error,
        created: invocation.call.created.clone(),
    })
}
