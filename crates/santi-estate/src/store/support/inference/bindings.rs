use keel::{
    Tx,
    adapt::{Error, db::Sqlite},
    form,
};
use santi_model::catalog::{Definition, Kind, Record, selection::Directory};
use std::collections::BTreeSet;

pub(super) fn paths(rows: &[Record]) -> Result<BTreeSet<String>, Error> {
    let dir = Directory(rows);
    let mut out = BTreeSet::new();
    for row in rows {
        if let Definition::Preference { candidate } = row.definition {
            let (provider, family, scenario) = dir.scope(candidate).map_err(Error::Adapt)?;
            let name = |kind, id| {
                dir.find(kind, id)
                    .map_err(Error::Adapt)?
                    .name()
                    .ok_or_else(|| Error::Adapt("expected named scenario dimension".into()))
            };
            out.insert(format!(
                "{}/{}/{}",
                name(Kind::Provider, provider)?,
                name(Kind::Family, family)?,
                name(Kind::Scenario, scenario)?
            ));
        }
    }
    Ok(out)
}

pub(super) async fn protect(
    tx: &mut Tx<'_, Sqlite>,
    before: &BTreeSet<String>,
    after: &[Record],
    service: Option<&str>,
) -> Result<(), Error> {
    for path in before.difference(&paths(after)?) {
        if service == Some(path.as_str()) {
            return Err(Error::Adapt(format!(
                "scenario {path} referenced by service declaration"
            )));
        }
        for unit in ["Soul", "Strand"] {
            if let Some(row) = tx
                .one(&form(unit).when("provider", keel::Op::Eq, path))
                .await?
            {
                return Err(Error::Adapt(format!(
                    "scenario {path} referenced by {unit}/{}",
                    row.key()
                )));
            }
        }
    }
    Ok(())
}

pub(super) async fn legacy(
    tx: &mut Tx<'_, Sqlite>,
    definition: &Definition,
) -> Result<Vec<String>, Error> {
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
