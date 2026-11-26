mod entries;

use super::{Store, parse, read, target};
use keel::{Op, Rank, form};
use santi_model::{message, strand, stream};

impl Store {
    pub async fn tail(&self, strand: &str) -> Result<Option<stream::Tail>, String> {
        let Some(strand) = read::one(&self.core, "Strand", "tag", strand).await? else {
            return Ok(None);
        };
        let rows = self
            .core
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &strand.key().to_string())
                    .order("sequence", Rank::Desc)
                    .top(stream::RECORDS + 1),
            )
            .await
            .map_err(read::error)?;
        let omitted = rows.rows().len() > stream::RECORDS;
        let mut records = Vec::with_capacity(rows.rows().len().min(stream::RECORDS));
        for row in rows.rows().iter().take(stream::RECORDS) {
            let aimed = crate::store::entry::target(&self.core, row).await?;
            let target = read::text(&aimed, "tag")?;
            let kind = target::decode(read::text(row, "target_type")?)?;
            let sequence = read::int(row, "sequence")?;
            let created = read::text(row, "created")?.to_string();
            let record = match kind {
                strand::Target::Message => stream::Record::project(
                    sequence,
                    created,
                    kind,
                    &self.placed(row, &aimed).await?,
                ),
                strand::Target::Compact => stream::Record::project(
                    sequence,
                    created,
                    kind,
                    &self
                        .compact(target)
                        .await?
                        .ok_or_else(|| format!("compact {target} missing"))?,
                ),
                strand::Target::Thinking => stream::Record::project(
                    sequence,
                    created,
                    kind,
                    &self
                        .thinking(target)
                        .await?
                        .ok_or_else(|| format!("thinking {target} missing"))?,
                ),
                strand::Target::Call => stream::Record::project(
                    sequence,
                    created,
                    kind,
                    &self
                        .call(target)
                        .await?
                        .ok_or_else(|| format!("tool call {target} missing"))?,
                ),
                strand::Target::Result => stream::Record::project(
                    sequence,
                    created,
                    kind,
                    &self
                        .reply(target)
                        .await?
                        .ok_or_else(|| format!("tool result {target} missing"))?,
                ),
            }
            .map_err(|error| error.to_string())?;
            records.push(record);
        }
        let tail = stream::Tail::newest(records, omitted);
        tail.bounded()?;
        Ok(Some(tail))
    }

    pub async fn events(&self, strand: &str) -> Result<Vec<message::Event>, String> {
        let strand = read::one(&self.core, "Strand", "tag", strand)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let entries = self
            .core
            .ask(
                &form("StrandEntry")
                    .when("strand", Op::Eq, &strand.key().to_string())
                    .when("target_type", Op::Eq, "message")
                    .order("sequence", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        let mut events = Vec::new();
        for entry in entries.rows() {
            let message = crate::store::entry::target(&self.core, entry).await?;
            let rows = self
                .core
                .ask(
                    &form("MessageEvent")
                        .when("message", Op::Eq, &message.key().to_string())
                        .order("created", Rank::Asc)
                        .order("tag", Rank::Asc),
                )
                .await
                .map_err(read::error)?;
            for row in rows.rows() {
                events.push(decode(self, row).await?);
            }
        }
        Ok(events)
    }

    pub async fn snapshot(&self, strand: &str) -> Result<Option<stream::Snapshot>, String> {
        let Some(strand) = self.strand(strand).await? else {
            return Ok(None);
        };
        Ok(Some(stream::Snapshot {
            messages: self.messages(&strand.id).await?,
            events: self.events(&strand.id).await?,
            turns: self.turns(&strand.id).await?,
            thinking: self.thinkings(&strand.id).await?,
            contexts: self.contexts(&strand.id).await?,
            calls: self.calls(&strand.id).await?,
            results: self.results(&strand.id).await?,
            compacts: self.compacts(&strand.id).await?,
            effects: self.effects(&strand.id).await?,
            errors: self
                .incidents(&santi_error::Scope::new("strand", &strand.id), 100)
                .await?,
            strand,
        }))
    }
}

async fn decode(store: &Store, row: &keel::Row) -> Result<message::Event, String> {
    Ok(message::Event {
        id: read::text(row, "tag")?.to_string(),
        message: read::related(&store.core, "Message", read::int(row, "message")?).await?,
        action: read::text(row, "action")?.to_string(),
        role: parse::role(read::text(row, "actor_type")?)?,
        actor: read::text(row, "actor")?.to_string(),
        base: read::int(row, "base_version")?,
        payload: serde_json::from_str(read::text(row, "payload")?)
            .map_err(|error| error.to_string())?,
        created: read::text(row, "created")?.to_string(),
    })
}
