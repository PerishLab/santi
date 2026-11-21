mod route;
use super::{Store, read};
use keel::adapt::db::Sqlite;
use keel::{Op, Tx, form};
use santi_model::inference::{Effective, Selection, Source};

impl Store {
    pub async fn inference(&self, tag: &str) -> Result<Option<Effective>, String> {
        let Some(turn) = read::one(&self.core, "Turn", "tag", tag).await? else {
            return Ok(None);
        };
        let key = turn.key().to_string();
        let Some(row) = read::one(&self.core, "Inference", "turn", &key).await? else {
            return Ok(None);
        };
        let mut source = serde_json::from_str::<Source>(read::text(&row, "source")?)
            .map_err(|error| format!("invalid inference source: {error}"))?;
        source.route = read::one(&self.core, "Route", "turn", &key)
            .await?
            .as_ref()
            .map(route::decode)
            .transpose()?
            .map(Box::new);
        Ok(Some(Effective {
            selection: source
                .selection
                .with(read::text(&row, "provider")?.to_owned()),
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
    effective
        .check()
        .map_err(|error| keel::adapt::Error::Adapt(error.into()))?;
    let Selection::Scenario(scenario) = &effective.selection else {
        return Err(keel::adapt::Error::Adapt(
            "new turn requires a scenario".into(),
        ));
    };
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
    let mut origins = effective.source.clone();
    if let Some(route) = origins.route.take() {
        let rows = crate::store::support::inference::snapshot(tx).await?;
        route::verify(&rows, effective, &route).map_err(keel::adapt::Error::Adapt)?;
        route::record(tx, &key, &route).await?;
    }
    let source = serde_json::to_string(&origins)
        .map_err(|error| keel::adapt::Error::Adapt(error.to_string()))?;
    let mut values = vec![
        ("provider", scenario.as_str()),
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
