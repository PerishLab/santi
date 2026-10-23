use crate::store::{Store, read};
use keel::adapt::db::Sqlite;
use keel::{Core, Op, form};
use santi_model::thinking;

impl Store {
    pub async fn chronicle(
        &self,
        tag: &str,
        round: &thinking::Round,
    ) -> Result<Option<thinking::Span>, String> {
        let record = serde_json::to_string(round).map_err(|error| error.to_string())?;
        let found = self
            .core
            .batch(async |tx| {
                let Some(row) = tx
                    .one(&form("ThinkingSpan").when("tag", Op::Eq, tag))
                    .await?
                else {
                    return Ok(false);
                };
                let key = row.key().to_string();
                if tx
                    .one(&form("Round").when("thinking", Op::Eq, &key))
                    .await?
                    .is_none()
                {
                    tx.put("Round", &[("record", record.as_str()), ("thinking", &key)])
                        .await?;
                }
                Ok(true)
            })
            .await
            .map_err(read::error)?;
        if found {
            self.thinking(tag).await
        } else {
            Ok(None)
        }
    }
}

pub(super) async fn rounded(
    core: &Core<Sqlite>,
    key: &str,
) -> Result<Option<thinking::Round>, String> {
    let Some(row) = read::one(core, "Round", "thinking", key).await? else {
        return Ok(None);
    };
    serde_json::from_str(read::text(&row, "record")?)
        .map(Some)
        .map_err(|error| format!("thinking round record unreadable: {error}"))
}
