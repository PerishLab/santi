use super::*;
use crate::store::write;

impl Writer<'_, '_> {
    pub(super) async fn insert(
        &mut self,
        message: Message<'_, '_>,
    ) -> Result<Written, keel::adapt::Error> {
        let tag = santi_model::tag("msg");
        let key = self
            .0
            .put(
                "Message",
                &[
                    ("tag", tag.as_str()),
                    ("actor_type", "system"),
                    ("actor", message.draft.actor),
                    ("kind", message.kind),
                    ("content", message.content),
                    ("state", "fixed"),
                    ("request", "true"),
                    ("created", message.draft.created),
                    ("updated", message.draft.created),
                ],
            )
            .await?;
        let strand = self
            .0
            .one(&form("Strand").when("id", Op::Eq, &message.strand.key().to_string()))
            .await?
            .ok_or_else(|| keel::adapt::Error::Missing("drain strand".into()))?;
        let sequence = write::append(
            self.0,
            write::Entry {
                strand: &strand,
                kind: "message",
                target: &tag,
                created: message.draft.created,
            },
        )
        .await?;
        Ok(Written { key, tag, sequence })
    }

    pub(super) async fn put_turn(
        &mut self,
        strand: &Row,
        draft: &DrainDraft<'_>,
        from: i64,
    ) -> Result<(), keel::adapt::Error> {
        let strand = strand.key().to_string();
        let from = from.to_string();
        let mut fields = vec![
            ("tag", draft.turn),
            ("trigger", trigger(&draft.trigger)),
            ("from", from.as_str()),
            ("created", draft.created),
            ("updated", draft.created),
            ("strand", strand.as_str()),
        ];
        if let Some(source) = draft.source {
            fields.push(("source", source));
        }
        self.0.put("Turn", &fields).await?;
        Ok(())
    }

    pub(super) async fn consume(
        &mut self,
        item: Assigned,
        draft: &DrainDraft<'_>,
    ) -> Result<(), keel::adapt::Error> {
        let metadata = item
            .pending
            .source_metadata
            .as_deref()
            .map(serde_json::from_str::<Value>)
            .transpose()
            .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
        let payload = json!({
            "kind": "inbox_drain",
            "inbox": item.pending.tag,
            "queued": item.pending.created,
            "drained_at": draft.created,
            "committing_turn_id": draft.turn,
            "message": item.message.tag,
            "seq": item.message.sequence,
            "source": {
                "type": item.pending.source_type,
                "ref": item.pending.source_ref,
                "metadata": metadata,
            }
        })
        .to_string();
        self.0
            .put(
                "MessageEvent",
                &[
                    ("tag", &santi_model::tag("mev")),
                    ("action", "insert"),
                    ("actor_type", "system"),
                    ("actor", draft.actor),
                    ("base_version", "1"),
                    ("payload", &payload),
                    ("created", draft.created),
                    ("message", &item.message.key.to_string()),
                ],
            )
            .await?;
        if let Some(slot) = self
            .0
            .one(&form("InboxSlot").when("inbox", Op::Eq, &item.pending.key.to_string()))
            .await?
        {
            self.0.unset("InboxSlot", slot.key(), &["inbox"]).await?;
            self.0
                .set("InboxSlot", slot.key(), &[("updated", draft.created)])
                .await?;
        }
        receipt::shift(
            self.0,
            super::super::ReceiptDraft {
                inbox: &item.pending.tag,
                state: receipt_model::State::Driving,
                turn: Some(draft.turn),
                incident: None,
                rebuilt: None,
                occurred: draft.created,
            },
        )
        .await?;
        self.0.end("StrandInbox", item.pending.key).await?;
        Ok(())
    }
}
