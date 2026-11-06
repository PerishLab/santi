use super::*;

pub struct Acceptance<'a> {
    pub replay: ReplayDraft<'a>,
    pub gate: usize,
    pub binding: Option<&'a santi_model::receipt::reply::Binding>,
}

impl Store {
    pub async fn accept_replay(
        &self,
        draft: InboxDraft<'_>,
        replay: ReplayDraft<'_>,
        gate: usize,
    ) -> Result<Accepted, String> {
        self.windowed(
            draft,
            Acceptance {
                replay,
                gate,
                binding: None,
            },
        )
        .await
    }

    pub async fn windowed(
        &self,
        draft: InboxDraft<'_>,
        acceptance: Acceptance<'_>,
    ) -> Result<Accepted, String> {
        let Acceptance {
            replay,
            gate,
            binding,
        } = acceptance;
        let content = serde_json::to_string(draft.content).map_err(|error| error.to_string())?;
        let metadata = draft
            .source
            .and_then(|source| source.metadata.as_ref())
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| error.to_string())?;
        let accepted = self
            .core
            .batch(async |tx| {
                let Shape {
                    unit,
                    owner,
                    identity,
                    value,
                    digest,
                } = replay_shape(tx, replay).await?;
                if let Some(row) = tx
                    .one(
                        &form(unit)
                            .when(owner.0, Op::Eq, &owner.1)
                            .when(identity, Op::Eq, value),
                    )
                    .await?
                {
                    if row.text("request_sha256") != Some(digest) {
                        return Err(keel::adapt::Error::Adapt(conflict(replay).into()));
                    }
                    let (strand, inbox) = receipt(tx, &row).await?;
                    return Ok((strand, inbox, false));
                }
                inbox::accept(
                    tx,
                    inbox::Acceptance {
                        draft: &draft,
                        gate,
                        content: &content,
                        metadata: metadata.as_deref(),
                    },
                )
                .await?;
                if let Some(binding) = binding {
                    super::super::reply::create(tx, draft.tag, binding).await?;
                }
                let receipt = read::need(tx, "InboxReceipt", "tag", draft.tag)
                    .await?
                    .to_string();
                let mut fields = vec![
                    (identity, value),
                    ("request_sha256", digest),
                    ("created", draft.created),
                    ("receipt", receipt.as_str()),
                ];
                fields.push((owner.0, owner.1.as_str()));
                tx.put(unit, &fields).await?;
                Ok((draft.strand.to_string(), draft.tag.to_string(), true))
            })
            .await
            .map_err(read::error)?;
        Ok(Accepted {
            receipt: ingest::Receipt {
                strand: accepted.0,
                inbox: accepted.1,
                warning: None,
            },
            inserted: accepted.2,
        })
    }
}
