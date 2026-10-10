use super::success::{Completion, CompletionDraft};
use super::{Store, read};

impl Store {
    pub async fn handoff(
        &self,
        draft: CompletionDraft<'_>,
        inbox: crate::store::InboxDraft<'_>,
    ) -> Result<Completion, String> {
        let tag = draft.turn.to_string();
        let event = self
            .core
            .batch(async |tx| complete(tx, draft, &inbox).await)
            .await
            .map_err(read::error)?;
        let turn = self
            .turn(&tag)
            .await?
            .ok_or_else(|| "completed turn missing".to_string())?;
        Ok(Completion { turn, event })
    }
}

pub(in crate::store::turn) async fn complete(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    draft: CompletionDraft<'_>,
    inbox: &crate::store::InboxDraft<'_>,
) -> Result<Option<santi_model::event::Event>, keel::adapt::Error> {
    check(tx, draft.turn, inbox.strand).await?;
    let content = serde_json::to_string(inbox.content)
        .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
    let metadata = inbox
        .source
        .and_then(|source| source.metadata.as_ref())
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
    let event = super::success::finish(tx, draft).await?;
    crate::store::inbox::write::accept(
        tx,
        crate::store::inbox::write::Acceptance {
            draft: inbox,
            gate: 500,
            content: &content,
            metadata: metadata.as_deref(),
        },
    )
    .await?;
    Ok(event)
}

pub(in crate::store::turn) async fn check(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    turn: &str,
    strand: &str,
) -> Result<(), keel::adapt::Error> {
    let row = super::Reader(tx).admitted(turn).await?;
    let strand = crate::store::read::need(tx, "Strand", "tag", strand).await?;
    if row.int("strand") != Some(strand) {
        return Err(keel::adapt::Error::Adapt(
            "handoff strand differs from turn".into(),
        ));
    }
    Ok(())
}
