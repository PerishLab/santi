use super::*;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

impl Store {
    pub async fn capture(&self, draft: Capture<'_>) -> Result<material::Context, String> {
        self.core
            .batch(async |tx| Writer(tx).capture(draft).await)
            .await
            .map_err(read::error)
    }

    pub async fn contextualize(&self, thinking: &str) -> Result<(), String> {
        self.core
            .batch(async |tx| {
                let span = read::need(tx, "ThinkingSpan", "tag", thinking).await?;
                let mut writer = Writer(tx);
                let turn = int(&writer.row("ThinkingSpan", span).await?, "turn")?;
                let rows = writer
                    .0
                    .ask(
                        &form("Context")
                            .when("turn", Op::Eq, &turn.to_string())
                            .order("round", Rank::Desc),
                    )
                    .await?;
                if let Some(row) = rows.rows().iter().find(|row| row.int("thinking").is_none()) {
                    writer
                        .0
                        .set("Context", row.key(), &[("thinking", &span.to_string())])
                        .await?;
                }
                Ok(())
            })
            .await
            .map_err(read::error)
    }

    pub async fn contexts(&self, strand: &str) -> Result<Vec<material::Context>, String> {
        self.core
            .batch(async |tx| {
                let mut writer = Writer(tx);
                let owner = writer.owner(Scope::Strand, strand).await?;
                let rows = writer.contexts(&owner).await?;
                let mut contexts = Vec::new();
                for row in rows {
                    contexts.push(writer.context(&row).await?);
                }
                contexts.sort_by(|a, b| {
                    (&a.created, &a.turn, a.round)
                        .cmp(&(&b.created, &b.turn, b.round))
                        .then_with(|| a.id.cmp(&b.id))
                });
                Ok(contexts)
            })
            .await
            .map_err(read::error)
    }

    pub async fn context(
        &self,
        strand: &str,
        id: &str,
    ) -> Result<Option<material::Context>, String> {
        self.core
            .batch(async |tx| {
                let mut writer = Writer(tx);
                let owner = writer.owner(Scope::Strand, strand).await?;
                match writer.find(&owner, id).await? {
                    Some(row) => writer.context(&row).await.map(Some),
                    None => Ok(None),
                }
            })
            .await
            .map_err(read::error)
    }

    pub async fn archived(&self, strand: &str, id: &str) -> Result<Vec<material::Package>, String> {
        self.core
            .batch(async |tx| {
                let mut writer = Writer(tx);
                let owner = writer.owner(Scope::Strand, strand).await?;
                let row = writer
                    .find(&owner, id)
                    .await?
                    .ok_or_else(|| Error::Missing("material context".into()))?;
                let uses = writer
                    .0
                    .ask(
                        &form("Usage")
                            .when("context", Op::Eq, &row.key().to_string())
                            .order("position", Rank::Asc),
                    )
                    .await?;
                let mut packages = Vec::new();
                for used in uses.rows() {
                    let mut package = writer.package(int(used, "revision")?).await?;
                    let origins: Vec<material::Origin> = decode(&text(used, "provenance")?)?;
                    if let Some(origin) = origins.first() {
                        package.material.provenance = origin.provenance.clone();
                    }
                    packages.push(package);
                }
                Ok(packages)
            })
            .await
            .map_err(read::error)
    }
}

impl Writer<'_, '_> {
    async fn capture(&mut self, draft: Capture<'_>) -> Result<material::Context, Error> {
        let turn = read::need(self.0, "Turn", "tag", draft.turn).await?;
        let strand = int(&self.row("Turn", turn).await?, "strand")?;
        let row = self.row("Strand", strand).await?;
        let soul = int(&row, "soul")?;
        if draft.selection.scope != Scope::Strand || draft.selection.owner != text(&row, "tag")? {
            return Err(Error::Adapt(
                "material selection conflicts with request strand".into(),
            ));
        }
        let digest = format!("{:x}", Sha256::digest(draft.prompt.as_bytes()));
        let context = self
            .0
            .put(
                "Context",
                &[
                    ("tag", draft.tag),
                    ("round", &draft.round.to_string()),
                    ("digest", &digest),
                    ("rendered", draft.rendered),
                    ("created", draft.created),
                    ("turn", &turn.to_string()),
                ],
            )
            .await?;
        for (position, component) in draft.components.iter().enumerate() {
            let key = format!("{}@{}", component.reference, component.revision);
            let revision = self
                .0
                .one(
                    &form("Revision")
                        .when("owner", Op::Eq, &soul.to_string())
                        .when("key", Op::Eq, &key),
                )
                .await?
                .ok_or_else(|| Error::Missing("material revision for context".into()))?;
            self.0
                .put(
                    "Usage",
                    &[
                        ("position", &position.to_string()),
                        ("provenance", &encode(&component.origins)?),
                        ("bytes", &component.bytes.to_string()),
                        ("context", &context.to_string()),
                        ("revision", &revision.key().to_string()),
                    ],
                )
                .await?;
        }
        let row = self.row("Context", context).await?;
        self.context(&row).await
    }

    async fn context(&mut self, row: &Row) -> Result<material::Context, Error> {
        let turn = text(&self.row("Turn", int(row, "turn")?).await?, "tag")?;
        let thinking = match row.int("thinking") {
            Some(key) => Some(text(&self.row("ThinkingSpan", key).await?, "tag")?),
            None => None,
        };
        let uses = self
            .0
            .ask(
                &form("Usage")
                    .when("context", Op::Eq, &row.key().to_string())
                    .order("position", Rank::Asc),
            )
            .await?;
        let mut materials = Vec::new();
        for used in uses.rows() {
            let package = self.package(int(used, "revision")?).await?;
            materials.push(material::Component {
                reference: package.material.reference,
                revision: package.material.revision,
                entry: package.material.entry,
                origins: decode(&text(used, "provenance")?)?,
                bytes: usize::try_from(int(used, "bytes")?)
                    .map_err(|e| Error::Adapt(e.to_string()))?,
            });
        }
        Ok(material::Context {
            id: text(row, "tag")?,
            turn,
            round: usize::try_from(int(row, "round")?).map_err(|e| Error::Adapt(e.to_string()))?,
            thinking,
            created: text(row, "created")?,
            digest: text(row, "digest")?,
            materials,
        })
    }

    async fn contexts(&mut self, owner: &Owner) -> Result<Vec<Row>, Error> {
        let turns = self
            .0
            .ask(&form("Turn").when("strand", Op::Eq, &owner.key.to_string()))
            .await?;
        let mut found = BTreeMap::new();
        for turn in turns.rows() {
            let rows = self
                .0
                .ask(&form("Context").when("turn", Op::Eq, &turn.key().to_string()))
                .await?;
            for row in rows.rows() {
                found.insert(row.key(), row.clone());
            }
        }
        let entries = self
            .0
            .ask(&form("StrandEntry").when("strand", Op::Eq, &owner.key.to_string()))
            .await?;
        for entry in entries.rows() {
            if let Some(span) = entry.int("thinking") {
                let rows = self
                    .0
                    .ask(&form("Context").when("thinking", Op::Eq, &span.to_string()))
                    .await?;
                for row in rows.rows() {
                    found.insert(row.key(), row.clone());
                }
            }
        }
        Ok(found.into_values().collect())
    }

    async fn find(&mut self, owner: &Owner, id: &str) -> Result<Option<Row>, Error> {
        let row = self.0.one(&form("Context").when("tag", Op::Eq, id)).await?;
        let row = match row {
            Some(row) => Some(row),
            None => match self
                .0
                .one(&form("ThinkingSpan").when("tag", Op::Eq, id))
                .await?
            {
                Some(span) => {
                    self.0
                        .one(&form("Context").when("thinking", Op::Eq, &span.key().to_string()))
                        .await?
                }
                None => None,
            },
        };
        let Some(row) = row else { return Ok(None) };
        if int(&self.row("Turn", int(&row, "turn")?).await?, "strand")? == owner.key {
            return Ok(Some(row));
        }
        if let Some(span) = row.int("thinking")
            && self
                .0
                .one(
                    &form("StrandEntry")
                        .when("strand", Op::Eq, &owner.key.to_string())
                        .when("thinking", Op::Eq, &span.to_string()),
                )
                .await?
                .is_some()
        {
            return Ok(Some(row));
        }
        Err(Error::Adapt(
            "material context is not in this strand".into(),
        ))
    }
}
