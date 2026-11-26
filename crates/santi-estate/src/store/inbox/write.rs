use super::{Arrival, Notice, Offer, json, kind};
use keel::adapt::db::Sqlite;
use keel::{Row, Tx};
use std::collections::BTreeSet;

mod support;
use support::{adapt, held, linked, merged, source, tag};

pub(in crate::store) struct Acceptance<'a, 'draft> {
    pub draft: &'a Arrival<'draft>,
    pub gate: usize,
    pub content: &'a str,
    pub metadata: Option<&'a str>,
}

#[derive(Clone, Copy)]
struct Entry<'a, 'draft> {
    draft: Notice<'draft>,
    content: &'a str,
    metadata: Option<&'a str>,
}

struct Writer<'a, 'tx> {
    tx: &'a mut Tx<'tx, Sqlite>,
}

pub(in crate::store) async fn accept(
    tx: &mut Tx<'_, Sqlite>,
    acceptance: Acceptance<'_, '_>,
) -> Result<(), keel::adapt::Error> {
    let draft = acceptance.draft;
    let mut writer = Writer { tx };
    let strand = crate::store::read::need(writer.tx, "Strand", "tag", draft.strand).await?;
    writer.capacity(strand, acceptance.gate).await?;
    let strand = strand.to_string();
    let mut fields = vec![
        ("tag", draft.tag),
        ("kind", kind(&draft.kind)),
        ("content", acceptance.content),
        ("created", draft.created),
        ("strand", strand.as_str()),
    ];
    if let Some(source) = draft.source {
        fields.push(("source_type", source.kind.as_str()));
        if let Some(reference) = source.source.as_deref() {
            fields.push(("source_ref", reference));
        }
        if let Some(metadata) = acceptance.metadata {
            fields.push(("source_metadata", metadata));
        }
    }
    writer.tx.put("StrandInbox", &fields).await?;
    super::receipt::accept(writer.tx, draft.tag, &strand, draft.created).await
}

pub(in crate::store) async fn offer(
    tx: &mut Tx<'_, Sqlite>,
    draft: Notice<'_>,
    gate: usize,
) -> Result<Offer, keel::adapt::Error> {
    let mut writer = Writer { tx };
    if draft.revision < 1 {
        return Err(adapt("inbox notice revision must be positive".into()));
    }
    let content = json(draft.content).map_err(adapt)?;
    let metadata = draft
        .source
        .metadata
        .as_ref()
        .map(json)
        .transpose()
        .map_err(adapt)?;
    let causes = draft.causes.iter().cloned().collect::<BTreeSet<_>>();
    let strand = crate::store::read::need(writer.tx, "Strand", "tag", draft.strand).await?;
    let key = strand.to_string();
    let slot = writer
        .tx
        .one(
            &keel::form("InboxSlot")
                .when("strand", keel::Op::Eq, &key)
                .when("key", keel::Op::Eq, draft.key),
        )
        .await?;
    if let Some(slot) = slot.as_ref()
        && let Some(offer) = writer
            .settle(
                slot,
                Entry {
                    draft,
                    content: &content,
                    metadata: metadata.as_deref(),
                },
                &causes,
            )
            .await?
    {
        return Ok(offer);
    }
    writer.capacity(strand, gate).await?;
    let causes = json(&causes).map_err(adapt)?;
    let notice = Entry {
        draft,
        content: &content,
        metadata: metadata.as_deref(),
    };
    let inbox = writer.insert(&key, notice, &causes).await?;
    super::receipt::accept(writer.tx, draft.tag, &key, draft.created).await?;
    match slot {
        Some(slot) => writer.update(&slot, draft, inbox).await?,
        None => writer.create(draft, &key, inbox).await?,
    }
    Ok(Offer {
        inbox: Some(draft.tag.to_string()),
        inserted: true,
    })
}

impl Writer<'_, '_> {
    async fn capacity(&mut self, strand: i64, gate: usize) -> Result<(), keel::adapt::Error> {
        let pending = self
            .tx
            .ask(&keel::form("StrandInbox").when("strand", keel::Op::Eq, &strand.to_string()))
            .await?
            .rows()
            .len();
        if pending < gate {
            Ok(())
        } else {
            Err(keel::adapt::Error::Adapt(format!(
                "strand inbox is full ({pending} pending, gate {gate})"
            )))
        }
    }

    async fn settle(
        &mut self,
        slot: &Row,
        notice: Entry<'_, '_>,
        incoming: &BTreeSet<String>,
    ) -> Result<Option<Offer>, keel::adapt::Error> {
        let draft = notice.draft;
        let current = slot
            .int("revision")
            .ok_or_else(|| keel::adapt::Error::Adapt("slot revision missing".into()))?;
        let inbox = linked(self.tx, slot).await?;
        if draft.revision < current {
            return Ok(Some(held(inbox.as_ref())?));
        }
        if draft.revision == current {
            if slot.text("digest") != Some(draft.digest) {
                return Err(keel::adapt::Error::Adapt(
                    "inbox notice revision conflicts with its accepted payload".into(),
                ));
            }
            return Ok(Some(held(inbox.as_ref())?));
        }
        let Some(inbox) = inbox else {
            return Ok(None);
        };
        let causes = merged(&inbox, incoming)?;
        self.replace(&inbox, notice, &causes).await?;
        self.update(slot, draft, inbox.key()).await?;
        Ok(Some(Offer {
            inbox: Some(tag(&inbox)?),
            inserted: false,
        }))
    }

    async fn insert(
        &mut self,
        strand: &str,
        notice: Entry<'_, '_>,
        causes: &str,
    ) -> Result<i64, keel::adapt::Error> {
        let draft = notice.draft;
        let revision = draft.revision.to_string();
        let mut fields = vec![
            ("tag", draft.tag),
            ("kind", "santi_system"),
            ("content", notice.content),
            ("source_type", draft.source.kind.as_str()),
            ("coalesce_key", draft.key),
            ("coalesce_revision", revision.as_str()),
            ("coalesce_causes", causes),
            ("created", draft.created),
            ("strand", strand),
        ];
        source(&mut fields, draft.source.source.as_deref(), notice.metadata);
        self.tx.put("StrandInbox", &fields).await
    }

    async fn replace(
        &mut self,
        inbox: &Row,
        notice: Entry<'_, '_>,
        causes: &BTreeSet<String>,
    ) -> Result<(), keel::adapt::Error> {
        let draft = notice.draft;
        if inbox.text("coalesce_key") != Some(draft.key) {
            return Err(keel::adapt::Error::Adapt(
                "inbox slot points to a mismatched pending item".into(),
            ));
        }
        let causes = json(causes).map_err(adapt)?;
        let revision = draft.revision.to_string();
        let mut fields = vec![
            ("content", notice.content),
            ("source_type", draft.source.kind.as_str()),
            ("coalesce_revision", revision.as_str()),
            ("coalesce_causes", causes.as_str()),
        ];
        source(&mut fields, draft.source.source.as_deref(), notice.metadata);
        self.tx.set("StrandInbox", inbox.key(), &fields).await?;
        let mut absent = Vec::new();
        if draft.source.source.is_none() {
            absent.push("source_ref");
        }
        if draft.source.metadata.is_none() {
            absent.push("source_metadata");
        }
        if !absent.is_empty() {
            self.tx.unset("StrandInbox", inbox.key(), &absent).await?;
        }
        Ok(())
    }

    async fn create(
        &mut self,
        draft: Notice<'_>,
        strand: &str,
        inbox: i64,
    ) -> Result<(), keel::adapt::Error> {
        self.tx
            .put(
                "InboxSlot",
                &[
                    ("key", draft.key),
                    ("revision", &draft.revision.to_string()),
                    ("digest", draft.digest),
                    ("updated", draft.created),
                    ("strand", strand),
                    ("inbox", &inbox.to_string()),
                ],
            )
            .await?;
        Ok(())
    }

    async fn update(
        &mut self,
        slot: &Row,
        draft: Notice<'_>,
        inbox: i64,
    ) -> Result<(), keel::adapt::Error> {
        self.tx
            .set(
                "InboxSlot",
                slot.key(),
                &[
                    ("revision", &draft.revision.to_string()),
                    ("digest", draft.digest),
                    ("updated", draft.created),
                    ("inbox", &inbox.to_string()),
                ],
            )
            .await
    }
}
