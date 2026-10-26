use super::*;
use crate::store::{Accepted, InboxDraft, inbox};
use keel::{Tx, adapt::db::Sqlite};
use santi_model::{ingest, message};

impl Store {
    pub async fn accept_alarm(
        &self,
        tag: &str,
        occurrence: &alarm::Occurrence,
    ) -> Result<Accepted, String> {
        if occurrence.generation < 1 || occurrence.minute < 0 {
            return Err("alarm occurrence is invalid".into());
        }
        self.core
            .batch(async |tx| {
                let row = tx
                    .one(&form("Alarm").when("tag", Op::Eq, tag))
                    .await?
                    .ok_or_else(|| fail("alarm not found or ended"))?;
                let query = form("AlarmDelivery")
                    .when("alarm", Op::Eq, &row.key().to_string())
                    .when("generation", Op::Eq, &occurrence.generation.to_string())
                    .when("minute", Op::Eq, &occurrence.minute.to_string());
                if let Some(delivery) = tx.one(&query).await? {
                    let (strand, inbox) = crate::store::inbox::receipt(tx, &delivery).await?;
                    return Ok(accepted(&strand, inbox, false));
                }
                if row.int("generation") != Some(occurrence.generation) {
                    return Err(fail("alarm generation conflicts"));
                }
                insert(tx, tag, occurrence, &row).await
            })
            .await
            .map_err(read::error)
    }
}

async fn insert(
    tx: &mut Tx<'_, Sqlite>,
    tag: &str,
    occurrence: &alarm::Occurrence,
    row: &Row,
) -> Result<Accepted, keel::adapt::Error> {
    let key = row
        .int("strand")
        .ok_or_else(|| fail("alarm strand missing"))?
        .to_string();
    let strand = tx
        .one(&form("Strand").when("id", Op::Eq, &key))
        .await?
        .ok_or_else(|| fail("alarm strand missing"))?;
    let strand = strand
        .text("tag")
        .ok_or_else(|| fail("strand tag missing"))?;
    let content =
        message::Content::text(row.text("text").ok_or_else(|| fail("alarm text missing"))?);
    let source = ingest::Source::new("alarm")
        .with_ref(tag.to_string())
        .with_metadata(serde_json::json!({
            "endpoint": "cron",
            "generation": occurrence.generation,
            "invoked_minute": occurrence.minute,
        }));
    let encoded = serde_json::to_string(&content).map_err(|error| fail(&error.to_string()))?;
    let metadata =
        serde_json::to_string(&source.metadata).map_err(|error| fail(&error.to_string()))?;
    let inbox_tag = santi_model::tag("inbox");
    let now = santi_model::now();
    let draft = InboxDraft {
        tag: &inbox_tag,
        strand,
        kind: message::Kind::Text,
        content: &content,
        source: Some(&source),
        created: &now,
    };
    inbox::write::accept(
        tx,
        inbox::write::Acceptance {
            draft: &draft,
            gate: 500,
            content: &encoded,
            metadata: Some(&metadata),
        },
    )
    .await?;
    let receipt = crate::store::read::need(tx, "InboxReceipt", "tag", &inbox_tag).await?;
    tx.put(
        "AlarmDelivery",
        &[
            ("generation", &occurrence.generation.to_string()),
            ("minute", &occurrence.minute.to_string()),
            ("alarm", &row.key().to_string()),
            ("receipt", &receipt.to_string()),
        ],
    )
    .await?;
    Ok(accepted(strand, inbox_tag, true))
}

fn accepted(strand: &str, inbox: String, inserted: bool) -> Accepted {
    Accepted {
        receipt: ingest::Receipt {
            strand: strand.into(),
            inbox,
            warning: None,
        },
        inserted,
    }
}

fn fail(text: &str) -> keel::adapt::Error {
    keel::adapt::Error::Adapt(text.into())
}
