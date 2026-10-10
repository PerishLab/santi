use super::success::{Completion, CompletionDraft};
use super::{Store, read};
use keel::{Op, form};

impl Store {
    pub async fn handoff(
        &self,
        draft: CompletionDraft<'_>,
        inbox: crate::store::InboxDraft<'_>,
    ) -> Result<Completion, String> {
        let tag = draft.turn.to_string();
        let content = serde_json::to_string(inbox.content).map_err(|error| error.to_string())?;
        let metadata = inbox
            .source
            .and_then(|source| source.metadata.as_ref())
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| error.to_string())?;
        let event = self
            .core
            .batch(async |tx| {
                let row = tx
                    .one(&form("Turn").when("tag", Op::Eq, &tag))
                    .await?
                    .ok_or_else(|| keel::adapt::Error::Missing(tag.clone()))?;
                let strand = crate::store::read::need(tx, "Strand", "tag", inbox.strand).await?;
                if row.int("strand") != Some(strand) {
                    return Err(keel::adapt::Error::Adapt(
                        "handoff strand differs from turn".into(),
                    ));
                }
                let event = super::success::finish(tx, draft).await?;
                crate::store::inbox::write::accept(
                    tx,
                    crate::store::inbox::write::Acceptance {
                        draft: &inbox,
                        gate: 500,
                        content: &content,
                        metadata: metadata.as_deref(),
                    },
                )
                .await?;
                Ok(event)
            })
            .await
            .map_err(read::error)?;
        let turn = self
            .turn(&tag)
            .await?
            .ok_or_else(|| "completed turn missing".to_string())?;
        Ok(Completion { turn, event })
    }
}
