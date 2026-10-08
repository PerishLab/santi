use super::{Begun, DrainDraft, Opening, Store, read, receipt};
use keel::adapt::db::Sqlite;
use keel::{Op, Rank, Row, Tx, form};
use santi_model::receipt as receipt_model;
use serde_json::{Value, json};

mod codec;
mod recovery;
mod types;
mod writer;
use codec::{Pending, aggregate, decode, trigger};
use types::{Assigned, Message, Opened, Written};

struct Writer<'a, 'tx>(&'a mut Tx<'tx, Sqlite>);

pub(super) async fn open(
    store: &Store,
    draft: DrainDraft<'_>,
    retry: bool,
) -> Result<Opening, String> {
    let turn = draft.turn.to_string();
    let opened = store
        .core
        .batch(async |tx| open_in(tx, draft, retry).await)
        .await
        .map_err(read::error)?;
    match opened {
        Opened::Idle => Ok(Opening::Idle),
        Opened::Running(tag) => {
            let turn = store
                .turn(&tag)
                .await?
                .ok_or_else(|| "running turn missing".to_string())?;
            Ok(Opening::Running(turn))
        }
        Opened::Started(messages) => {
            let turn = store
                .turn(&turn)
                .await?
                .ok_or_else(|| "started turn missing".to_string())?;
            let mut drained = Vec::with_capacity(messages.len());
            for tag in messages {
                drained.push(
                    store
                        .message(&tag)
                        .await?
                        .ok_or_else(|| format!("drained message {tag} missing"))?,
                );
            }
            Ok(Opening::Started(Begun { turn, drained }))
        }
    }
}

async fn open_in(
    tx: &mut Tx<'_, Sqlite>,
    draft: DrainDraft<'_>,
    retry: bool,
) -> Result<Opened, keel::adapt::Error> {
    let strand = tx
        .one(&form("Strand").when("tag", Op::Eq, draft.strand))
        .await?
        .ok_or_else(|| keel::adapt::Error::Missing(draft.strand.into()))?;
    if let Some(tag) = running(tx, strand.key()).await? {
        return Ok(Opened::Running(tag));
    }
    let recovered = if retry {
        recovery::collect(tx, &strand).await?
    } else {
        recovery::Recovery::default()
    };
    let pending = pending(tx, strand.key()).await?;
    if pending.is_empty() && recovered.receipts.is_empty() {
        return Ok(Opened::Idle);
    }
    let (notices, regular): (Vec<_>, Vec<_>) = pending
        .into_iter()
        .partition(|pending| pending.coalesce_key.is_some());
    let mut messages = Vec::new();
    let mut assigned = Vec::new();
    let mut writer = Writer(tx);
    for pending in regular {
        let message = writer
            .insert(Message {
                strand: &strand,
                kind: &pending.kind,
                content: &pending.content,
                draft: &draft,
            })
            .await?;
        messages.push(message.clone());
        assigned.push(Assigned { pending, message });
    }
    if !notices.is_empty() {
        let content = aggregate(&notices, draft.created)?;
        let message = writer
            .insert(Message {
                strand: &strand,
                kind: "santi_system",
                content: &content,
                draft: &draft,
            })
            .await?;
        for pending in notices {
            assigned.push(Assigned {
                pending,
                message: message.clone(),
            });
        }
        messages.push(message);
    }
    let from = messages
        .last()
        .map(|message| message.sequence)
        .or(recovered.from)
        .ok_or_else(|| keel::adapt::Error::Adapt("drain produced no requests".into()))?;
    writer.put_turn(&strand, &draft, from).await?;
    recovery::bind(writer.0, recovered.receipts, &draft).await?;
    for item in assigned {
        writer.consume(item, &draft).await?;
    }
    Ok(Opened::Started(
        messages.into_iter().map(|message| message.tag).collect(),
    ))
}

async fn running(
    tx: &mut Tx<'_, Sqlite>,
    strand: i64,
) -> Result<Option<String>, keel::adapt::Error> {
    let turns = tx
        .ask(
            &form("Turn")
                .when("strand", Op::Eq, &strand.to_string())
                .order("created", Rank::Desc)
                .order("tag", Rank::Desc),
        )
        .await?;
    for turn in turns.rows() {
        let key = turn.key().to_string();
        let completed = tx
            .one(&form("TurnCompletion").when("turn", Op::Eq, &key))
            .await?
            .is_some();
        let failed = tx
            .one(&form("TurnFailure").when("turn", Op::Eq, &key))
            .await?
            .is_some();
        if !completed && !failed {
            return turn
                .text("tag")
                .map(str::to_string)
                .map(Some)
                .ok_or_else(|| keel::adapt::Error::Adapt("turn tag missing".into()));
        }
    }
    Ok(None)
}

async fn pending(tx: &mut Tx<'_, Sqlite>, strand: i64) -> Result<Vec<Pending>, keel::adapt::Error> {
    let rows = tx
        .ask(
            &form("StrandInbox")
                .when("strand", Op::Eq, &strand.to_string())
                .order("created", Rank::Asc)
                .order("tag", Rank::Asc),
        )
        .await?;
    rows.rows().iter().map(decode).collect()
}
