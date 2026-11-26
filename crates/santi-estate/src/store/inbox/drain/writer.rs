use super::*;
use crate::store::write;

impl Writer<'_, '_> {
    pub(super) async fn insert(
        &mut self,
        message: Message<'_, '_>,
    ) -> Result<Written, keel::adapt::Error> {
        self.named(message, &santi_model::tag("msg")).await
    }

    pub(super) async fn named(
        &mut self,
        message: Message<'_, '_>,
        tag: &str,
    ) -> Result<Written, keel::adapt::Error> {
        let tag = tag.to_string();
        let owner = message.strand.key().to_string();
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
                    ("strand", owner.as_str()),
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
                target: key,
                created: message.draft.created,
            },
        )
        .await?;
        Ok(Written { key, tag, sequence })
    }

    pub(super) async fn turn(
        &mut self,
        strand: &Row,
        draft: &Drain<'_>,
        from: i64,
    ) -> Result<(), keel::adapt::Error> {
        let strand = strand.key().to_string();
        let from = from.to_string();
        let fields = vec![
            ("tag", draft.turn),
            ("trigger", trigger(&draft.trigger)),
            ("from", from.as_str()),
            ("created", draft.created),
            ("updated", draft.created),
            ("strand", strand.as_str()),
        ];
        self.0.put("Turn", &fields).await?;
        Ok(())
    }

    pub(super) async fn consume(
        &mut self,
        item: Assigned,
        draft: &Drain<'_>,
    ) -> Result<(), keel::adapt::Error> {
        super::super::reply::attach(self.0, &item.pending.tag, item.message.key).await?;
        let metadata = item
            .pending
            .metadata
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
                "type": item.pending.origin,
                "ref": item.pending.reference,
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
            super::super::Receipt {
                inbox: &item.pending.tag,
                state: model::State::Driving,
                turn: Some(draft.turn),
                incident: None,
                occurred: draft.created,
            },
        )
        .await?;
        self.0.end("StrandInbox", item.pending.key).await?;
        Ok(())
    }
}
