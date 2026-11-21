use super::{bindings, catalog::snapshot};
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
    service: Option<&str>,
) -> Result<Record, Error> {
    definition.validate().map_err(Error::Adapt)?;
    let kind = definition.kind();
    let mut rows = snapshot(tx).await?;
    let paths = bindings::paths(&rows)?;
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
            let uses = bindings::legacy(tx, &old.definition).await?;
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
    bindings::protect(tx, &paths, &rows, service).await?;
    Ok(record)
}

pub(super) async fn delete(
    tx: &mut Tx<'_, Sqlite>,
    expected: Record,
    service: Option<&str>,
) -> Result<(), Error> {
    let mut rows = snapshot(tx).await?;
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
    uses.extend(bindings::legacy(tx, &expected.definition).await?);
    if !uses.is_empty() {
        return Err(Error::Adapt(format!(
            "source referenced by {}",
            uses.join(", ")
        )));
    }
    let paths = bindings::paths(&rows)?;
    rows.retain(|r| r.definition.kind() != kind || r.id != expected.id);
    bindings::protect(tx, &paths, &rows, service).await?;
    tx.end(kind.unit(), expected.id).await
}
