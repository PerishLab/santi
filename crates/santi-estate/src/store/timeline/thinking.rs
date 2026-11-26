use crate::store::{Store, read, write};
use keel::{Op, Rank, form};
use santi_model::thinking;

pub struct Thinking<'a> {
    pub tag: &'a str,
    pub turn: &'a str,
    pub response: Option<&'a str>,
    pub created: &'a str,
}

impl Store {
    pub async fn ponder(&self, draft: Thinking<'_>) -> Result<thinking::Span, String> {
        self.core
            .batch(async |tx| {
                let turn = tx
                    .one(&form("Turn").when("tag", Op::Eq, draft.turn))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing(draft.turn.into()))?;
                let strand = turn
                    .int("strand")
                    .ok_or_else(|| keel::adapt::Error::Adapt("turn strand missing".into()))?;
                let strand = tx
                    .one(&form("Strand").when("id", Op::Eq, &strand.to_string()))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing("turn strand".into()))?;
                let key = turn.key().to_string();
                let mut fields = vec![
                    ("tag", draft.tag),
                    ("created", draft.created),
                    ("updated", draft.created),
                    ("turn", key.as_str()),
                ];
                if let Some(response) = draft.response {
                    fields.push(("response", response));
                }
                let thinking = tx.put("ThinkingSpan", &fields).await?;
                write::append(
                    tx,
                    write::Entry {
                        strand: &strand,
                        kind: "thinking",
                        target: thinking,
                        created: draft.created,
                    },
                )
                .await?;
                Ok(())
            })
            .await
            .map_err(read::error)?;
        self.thinking(draft.tag)
            .await?
            .ok_or_else(|| "created thinking span missing".to_string())
    }

    pub async fn refine(
        &self,
        tag: &str,
        response: Option<&str>,
        summary: Option<&str>,
        updated: &str,
    ) -> Result<Option<thinking::Span>, String> {
        let mut fields = vec![("updated", updated)];
        if let Some(response) = response {
            fields.push(("response", response));
        }
        if let Some(summary) = summary {
            fields.push(("summary", summary));
        }
        let found = self
            .core
            .batch(async |tx| {
                let Some(row) = tx
                    .one(&form("ThinkingSpan").when("tag", Op::Eq, tag))
                    .await?
                else {
                    return Ok(false);
                };
                let key = row.key().to_string();
                let complete = tx
                    .one(&form("ThinkingCompletion").when("thinking", Op::Eq, &key))
                    .await?;
                let failed = tx
                    .one(&form("ThinkingFailure").when("thinking", Op::Eq, &key))
                    .await?;
                if complete.is_none() && failed.is_none() {
                    tx.set("ThinkingSpan", row.key(), &fields).await?;
                }
                Ok(true)
            })
            .await
            .map_err(read::error)?;
        if found {
            self.thinking(tag).await
        } else {
            Ok(None)
        }
    }

    pub async fn conclude(
        &self,
        tag: &str,
        reason: thinking::Reason,
        finished: &str,
    ) -> Result<Option<thinking::Span>, String> {
        finish(self, tag, Ok(reason), finished).await
    }

    pub async fn falter(
        &self,
        tag: &str,
        error: &str,
        finished: &str,
    ) -> Result<Option<thinking::Span>, String> {
        finish(self, tag, Err(error), finished).await
    }

    pub async fn thinking(&self, tag: &str) -> Result<Option<thinking::Span>, String> {
        let Some(row) = read::one(&self.core, "ThinkingSpan", "tag", tag).await? else {
            return Ok(None);
        };
        decode(self, &row).await.map(Some)
    }

    pub async fn thinkings(&self, strand: &str) -> Result<Vec<thinking::Span>, String> {
        let strand = read::one(&self.core, "Strand", "tag", strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let entries = self
            .core
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &strand.key().to_string())
                    .when("target_type", Op::Eq, "thinking")
                    .order("sequence", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        let mut spans = Vec::with_capacity(entries.rows().len());
        for entry in entries.rows() {
            let row = crate::store::entry::target(&self.core, entry).await?;
            spans.push(decode(self, &row).await?);
        }
        Ok(spans)
    }

    pub async fn thought(&self, turn: &str) -> Result<Vec<thinking::Span>, String> {
        let turn = read::one(&self.core, "Turn", "tag", turn)
            .await?
            .ok_or_else(|| "turn not found".to_string())?;
        let rows = self
            .core
            .ask(
                &form("ThinkingSpan")
                    .when("turn", Op::Eq, &turn.key().to_string())
                    .order("created", Rank::Asc)
                    .order("tag", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        let mut spans = Vec::with_capacity(rows.rows().len());
        for row in rows.rows() {
            spans.push(decode(self, row).await?);
        }
        Ok(spans)
    }
}

async fn finish(
    store: &Store,
    tag: &str,
    outcome: Result<thinking::Reason, &str>,
    finished: &str,
) -> Result<Option<thinking::Span>, String> {
    let found = store
        .core
        .batch(async |tx| {
            let Some(row) = tx
                .one(&form("ThinkingSpan").when("tag", Op::Eq, tag))
                .await?
            else {
                return Ok(false);
            };
            let key = row.key().to_string();
            let complete = tx
                .one(&form("ThinkingCompletion").when("thinking", Op::Eq, &key))
                .await?;
            let failed = tx
                .one(&form("ThinkingFailure").when("thinking", Op::Eq, &key))
                .await?;
            if complete.is_some() || failed.is_some() {
                return Ok(true);
            }
            match outcome {
                Ok(reason) => {
                    tx.put(
                        "ThinkingCompletion",
                        &[
                            ("reason", reason.encode()),
                            ("finished", finished),
                            ("thinking", &key),
                        ],
                    )
                    .await?;
                }
                Err(error) => {
                    tx.put(
                        "ThinkingFailure",
                        &[("error", error), ("finished", finished), ("thinking", &key)],
                    )
                    .await?;
                }
            }
            tx.set("ThinkingSpan", row.key(), &[("updated", finished)])
                .await?;
            Ok(true)
        })
        .await
        .map_err(read::error)?;
    if found {
        store.thinking(tag).await
    } else {
        Ok(None)
    }
}

pub(crate) async fn decode(store: &Store, row: &keel::Row) -> Result<thinking::Span, String> {
    let key = row.key().to_string();
    let complete = read::one(&store.core, "ThinkingCompletion", "thinking", &key).await?;
    let failed = read::one(&store.core, "ThinkingFailure", "thinking", &key).await?;
    let outcome = match (complete, failed) {
        (None, None) => Outcome {
            state: thinking::State::Running,
            reason: None,
            error: None,
            finished: None,
        },
        (Some(done), None) => Outcome {
            state: thinking::State::Completed,
            reason: Some(reason(read::text(&done, "reason")?)?),
            error: None,
            finished: Some(read::text(&done, "finished")?.to_string()),
        },
        (None, Some(failed)) => Outcome {
            state: thinking::State::Failed,
            reason: None,
            error: Some(read::text(&failed, "error")?.to_string()),
            finished: Some(read::text(&failed, "finished")?.to_string()),
        },
        (Some(_), Some(_)) => return Err("thinking span has conflicting outcomes".to_string()),
    };
    Ok(thinking::Span {
        id: read::text(row, "tag")?.to_string(),
        turn: read::related(&store.core, "Turn", read::int(row, "turn")?).await?,
        response: row.text("response").map(str::to_string),
        state: outcome.state,
        summary: row.text("summary").map(str::to_string),
        reason: outcome.reason,
        error: outcome.error,
        created: read::text(row, "created")?.to_string(),
        updated: read::text(row, "updated")?.to_string(),
        finished: outcome.finished,
        round: super::round::rounded(&store.core, &key).await?,
    })
}

struct Outcome {
    state: thinking::State,
    reason: Option<thinking::Reason>,
    error: Option<String>,
    finished: Option<String>,
}

fn reason(value: &str) -> Result<thinking::Reason, String> {
    match value {
        "spoke" => Ok(thinking::Reason::Spoke),
        "called" => Ok(thinking::Reason::Called),
        "finished" => Ok(thinking::Reason::Finished),
        value => Err(format!("unknown thinking reason {value}")),
    }
}
