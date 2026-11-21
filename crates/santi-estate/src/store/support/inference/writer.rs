use super::catalog::snapshot;
use keel::{
    Tx,
    adapt::{Error, db::Sqlite},
    form,
};
use santi_model::catalog::{Definition, Record, selection, validate};
use serde_json::Value;
use std::collections::BTreeMap;

fn cells(definition: &Definition) -> Result<BTreeMap<String, String>, Error> {
    let value = serde_json::to_value(definition).map_err(|e| Error::Adapt(e.to_string()))?;
    let values = value
        .as_object()
        .ok_or_else(|| Error::Adapt("invalid source definition".into()))?;
    values
        .iter()
        .filter(|(k, _)| k.as_str() != "kind")
        .map(|(k, v)| {
            let text = match v {
                Value::String(s) => s.clone(),
                Value::Number(i) => i.to_string(),
                _ => return Err(Error::Adapt(format!("invalid source field {k}"))),
            };
            Ok((k.clone(), text))
        })
        .collect()
}

fn verify(rows: &[Record], expected: &Record) -> Result<(), Error> {
    let actual = selection::Directory(rows)
        .find(expected.definition.kind(), expected.id)
        .map_err(Error::Adapt)?;
    if actual != &expected.definition {
        return Err(Error::Adapt("source changed since read".into()));
    }
    Ok(())
}

pub(super) async fn write(
    tx: &mut Tx<'_, Sqlite>,
    expected: Option<Record>,
    definition: Definition,
) -> Result<Record, Error> {
    definition.validate().map_err(Error::Adapt)?;
    let kind = definition.kind();
    let mut rows = snapshot(tx).await?;
    let fields = cells(&definition)?;
    let fields: Vec<_> = fields
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let id = if let Some(old) = expected {
        if old.definition.kind() != kind {
            return Err(Error::Adapt("source kind cannot change".into()));
        }
        verify(&rows, &old)?;
        if old.definition == definition {
            return Ok(old);
        }
        if old.definition.name() != definition.name()
            || matches!(old.definition, Definition::Alias { .. })
        {
            let uses = bindings(tx, &old.definition).await?;
            if !uses.is_empty() {
                return Err(Error::Adapt(format!(
                    "source referenced by {}",
                    uses.join(", ")
                )));
            }
        }
        rows.retain(|r| r.definition.kind() != kind || r.id != old.id);
        tx.set(kind.unit(), old.id, &fields).await?;
        old.id
    } else {
        tx.put(kind.unit(), &fields).await?
    };
    let record = Record { id, definition };
    rows.push(record.clone());
    validate::graph(&rows).map_err(Error::Adapt)?;
    Ok(record)
}

pub(super) async fn delete(tx: &mut Tx<'_, Sqlite>, expected: Record) -> Result<(), Error> {
    let rows = snapshot(tx).await?;
    verify(&rows, &expected)?;
    let kind = expected.definition.kind();
    let mut uses = validate::dependents(&rows, kind, expected.id);
    if !matches!(
        kind,
        santi_model::catalog::Kind::Alias | santi_model::catalog::Kind::Preference
    ) {
        let edge = kind.unit().to_lowercase();
        for row in tx
            .ask(&form("Route").when(edge.as_str(), keel::Op::Eq, &expected.id.to_string()))
            .await?
            .rows()
        {
            uses.push(format!(
                "Route/{} (turn {})",
                row.key(),
                row.int("turn").unwrap_or_default()
            ));
        }
    }
    uses.extend(bindings(tx, &expected.definition).await?);
    if !uses.is_empty() {
        return Err(Error::Adapt(format!(
            "source referenced by {}",
            uses.join(", ")
        )));
    }
    tx.end(kind.unit(), expected.id).await
}

async fn bindings(tx: &mut Tx<'_, Sqlite>, definition: &Definition) -> Result<Vec<String>, Error> {
    let mut uses = Vec::new();
    if let Definition::Alias { name, .. } | Definition::Access { name, .. } = definition {
        for unit in ["Soul", "Strand", "Inference"] {
            let declared = tx
                .ask(&form(unit).when("provider", keel::Op::Eq, name))
                .await?;
            for row in declared.rows() {
                if unit == "Inference"
                    && let Some(turn) = row.int("turn")
                    && (crate::store::turn::terminal(tx, turn).await?
                        || tx
                            .one(&form("Route").when("turn", keel::Op::Eq, &turn.to_string()))
                            .await?
                            .is_some())
                {
                    continue;
                }
                uses.push(format!("{unit}/{}", row.key()))
            }
        }
    }
    Ok(uses)
}
