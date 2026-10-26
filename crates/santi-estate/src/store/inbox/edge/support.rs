use super::{ReplayDraft, WebhookDraft};
use keel::{Op, form};

#[derive(PartialEq, Eq)]
struct Identity<'a> {
    adaptor: &'a str,
    strategy: &'a str,
    credential: &'a str,
    soul: i64,
}

impl<'a> Identity<'a> {
    fn read(row: &'a keel::Row) -> Option<Self> {
        Some(Self {
            adaptor: row.text("adaptor")?,
            strategy: row.text("strategy")?,
            credential: row.text("credential")?,
            soul: row.int("soul")?,
        })
    }
}

pub(super) fn exact(row: &keel::Row, draft: &WebhookDraft<'_>, soul: i64) -> bool {
    Identity::read(row)
        == Some(Identity {
            adaptor: draft.adaptor,
            strategy: draft.strategy,
            credential: draft.credential,
            soul,
        })
}

pub(super) fn conflict(replay: ReplayDraft<'_>) -> &'static str {
    match replay {
        ReplayDraft::Webhook { .. } => "webhook delivery conflicts with an accepted payload",
        ReplayDraft::Downstream { .. } => "downstream request conflicts with an accepted payload",
    }
}

fn text(row: &keel::Row, field: &str) -> Result<String, keel::adapt::Error> {
    row.text(field)
        .map(str::to_string)
        .ok_or_else(|| keel::adapt::Error::Adapt(format!("replay {field} missing")))
}

pub(in crate::store) async fn receipt(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    delivery: &keel::Row,
) -> Result<(String, String), keel::adapt::Error> {
    let key = delivery
        .int("receipt")
        .ok_or_else(|| keel::adapt::Error::Adapt("delivery receipt missing".into()))?;
    let receipt = tx
        .one(&form("InboxReceipt").when("id", Op::Eq, &key.to_string()))
        .await?
        .ok_or_else(|| keel::adapt::Error::Missing("delivery receipt".into()))?;
    let strand = receipt
        .int("strand")
        .ok_or_else(|| keel::adapt::Error::Adapt("receipt strand missing".into()))?;
    let strand = tx
        .one(&form("Strand").when("id", Op::Eq, &strand.to_string()))
        .await?
        .ok_or_else(|| keel::adapt::Error::Missing("receipt strand".into()))?;
    Ok((text(&strand, "tag")?, text(&receipt, "tag")?))
}
