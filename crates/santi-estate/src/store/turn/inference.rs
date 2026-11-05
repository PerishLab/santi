use super::{Store, read};
use keel::adapt::db::Sqlite;
use keel::{Op, Tx, form};
use santi_model::inference::{Effective, Source};

impl Store {
    pub async fn inference(&self, tag: &str) -> Result<Option<Effective>, String> {
        let Some(turn) = read::one(&self.core, "Turn", "tag", tag).await? else {
            return Ok(None);
        };
        let key = turn.key().to_string();
        let Some(row) = read::one(&self.core, "Inference", "turn", &key).await? else {
            return Ok(None);
        };
        let source = serde_json::from_str::<Source>(read::text(&row, "source")?)
            .map_err(|error| format!("invalid inference source: {error}"))?;
        Ok(Some(Effective {
            provider: read::text(&row, "provider")?.to_string(),
            model: read::text(&row, "model")?.to_string(),
            reasoning_effort: row.text("reasoning_effort").map(str::to_string),
            source,
        }))
    }
}

pub(in crate::store) async fn record(
    tx: &mut Tx<'_, Sqlite>,
    tag: &str,
    effective: &Effective,
    created: &str,
) -> Result<(), keel::adapt::Error> {
    let key = read::need(tx, "Turn", "tag", tag).await?.to_string();
    if tx
        .one(&form("Inference").when("turn", Op::Eq, &key))
        .await?
        .is_some()
    {
        return Err(keel::adapt::Error::Adapt(format!(
            "turn {tag} inference already recorded"
        )));
    }
    let source = serde_json::to_string(&effective.source)
        .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
    let mut values = vec![
        ("provider", effective.provider.as_str()),
        ("model", effective.model.as_str()),
        ("source", source.as_str()),
        ("created", created),
        ("turn", key.as_str()),
    ];
    if let Some(effort) = effective.reasoning_effort.as_deref() {
        values.push(("reasoning_effort", effort));
    }
    tx.put("Inference", &values).await?;
    Ok(())
}
