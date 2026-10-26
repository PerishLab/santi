use super::{Store, read};
use keel::{Op, Rank, form};

#[derive(Clone, Copy)]
pub struct MaterialDraft<'a> {
    pub strand: &'a str,
    pub key: &'a str,
    pub length: u64,
    pub sha256: &'a str,
    pub occurred: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Material {
    pub strand: String,
    pub key: String,
    pub length: u64,
    pub sha256: String,
    pub created: String,
    pub updated: String,
}

impl Store {
    pub async fn record_material(&self, draft: MaterialDraft<'_>) -> Result<(), String> {
        let length = i64::try_from(draft.length).map_err(|error| error.to_string())?;
        self.core
            .batch(async |tx| {
                let strand = read::need(tx, "Strand", "tag", draft.strand).await?;
                let strand = strand.to_string();
                let length = length.to_string();
                match tx
                    .one(&form("Material").when("strand", Op::Eq, &strand).when(
                        "key",
                        Op::Eq,
                        draft.key,
                    ))
                    .await?
                {
                    Some(row) => {
                        tx.set(
                            "Material",
                            row.key(),
                            &[
                                ("length", length.as_str()),
                                ("sha256", draft.sha256),
                                ("updated", draft.occurred),
                            ],
                        )
                        .await
                    }
                    None => tx
                        .put(
                            "Material",
                            &[
                                ("key", draft.key),
                                ("length", length.as_str()),
                                ("sha256", draft.sha256),
                                ("created", draft.occurred),
                                ("updated", draft.occurred),
                                ("strand", strand.as_str()),
                            ],
                        )
                        .await
                        .map(|_| ()),
                }
            })
            .await
            .map_err(read::error)
    }

    pub async fn end_material(&self, strand: &str, key: &str) -> Result<bool, String> {
        self.core
            .batch(async |tx| {
                let strand = read::need(tx, "Strand", "tag", strand).await?;
                let Some(row) = tx
                    .one(
                        &form("Material")
                            .when("strand", Op::Eq, &strand.to_string())
                            .when("key", Op::Eq, key),
                    )
                    .await?
                else {
                    return Ok(false);
                };
                tx.end("Material", row.key()).await?;
                Ok(true)
            })
            .await
            .map_err(read::error)
    }

    pub async fn materials(&self, strand: &str) -> Result<Vec<Material>, String> {
        let Some(row) = read::one(&self.core, "Strand", "tag", strand).await? else {
            return Ok(Vec::new());
        };
        let rows = self
            .core
            .ask(
                &form("Material")
                    .when("strand", Op::Eq, &row.key().to_string())
                    .order("key", Rank::Asc),
            )
            .await
            .map_err(read::error)?;
        rows.rows()
            .iter()
            .map(|row| {
                Ok(Material {
                    strand: strand.to_string(),
                    key: read::text(row, "key")?.to_string(),
                    length: u64::try_from(read::int(row, "length")?)
                        .map_err(|error| error.to_string())?,
                    sha256: read::text(row, "sha256")?.to_string(),
                    created: read::text(row, "created")?.to_string(),
                    updated: read::text(row, "updated")?.to_string(),
                })
            })
            .collect()
    }
}
