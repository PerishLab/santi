use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use super::kind::{Key, Kind, ORDER, model, name, rank, text};

fn key(row: &Value) -> Result<Key> {
    let r = rank(row)?;
    let kind = ORDER[r];
    let obj = row.as_object().context("expected definition")?;
    ensure!(
        obj.len() == Kind(kind).fields().len() + 1,
        "unexpected fields on {kind}"
    );
    for &field in Kind(kind).fields() {
        if Kind(kind).refs().contains(&field) {
            Kind(field).parts(&row[field])?;
            continue;
        }
        let s = row[field].as_str().context("missing catalog text field")?;
        let valid = match field {
            "name" => match kind {
                "provider" | "family" => crate::inference::word(s),
                "scenario" => crate::inference::scenario(s),
                _ => name(s),
            },
            "model" => model(s),
            "credential" => s.strip_prefix("custody://").is_some_and(name),
            "protocol" => matches!(
                s,
                "openai_responses" | "chat_completions" | "gemini_generate_content"
            ),
            "endpoint" => reqwest::Url::parse(s).is_ok_and(|u| {
                let http = matches!(u.scheme(), "https" | "http") && u.has_host();
                let anonymous = u.username().is_empty() && u.password().is_none();
                let plain = u.query().is_none() && u.fragment().is_none();
                http && anonymous && plain
            }),
            _ => text(s),
        };
        ensure!(valid, "invalid {kind} {field}");
    }
    let strings = match kind {
        "offering" => vec![
            row["access"].as_str().unwrap().into(),
            row["model"].as_str().unwrap().into(),
        ],
        "candidate" => {
            let mut v = Kind("offering").parts(&row["offering"])?;
            v.push(row["scenario"].as_str().unwrap().into());
            v
        }
        "preference" => Kind("candidate").parts(&row["candidate"])?,
        _ => vec![row["name"].as_str().unwrap().into()],
    };
    Ok((r, strings))
}

#[derive(Default)]
pub(super) struct Graph(BTreeMap<Key, Value>);

impl Graph {
    pub fn insert(&mut self, row: Value) -> Result<()> {
        let k = key(&row)?;
        if let Some(old) = self.0.get(&k) {
            ensure!(
                old == &row,
                "conflicting {} definition for {:?}; not replacing it",
                ORDER[k.0],
                k.1
            );
        } else {
            self.0.insert(k, row);
        }
        Ok(())
    }

    pub fn rows(&self) -> impl Iterator<Item = &Value> {
        self.0.values()
    }

    pub fn validate(&self) -> Result<()> {
        let mut names = BTreeSet::new();
        let mut scopes = BTreeSet::new();
        let mut bindings = BTreeMap::new();
        for ((r, _), row) in &self.0 {
            let kind = ORDER[*r];
            for &f in Kind(kind).refs() {
                ensure!(
                    self.0.contains_key(&Kind(f).reference(&row[f])?),
                    "missing {f} referenced by {kind}"
                );
            }
            if matches!(kind, "access" | "alias") {
                ensure!(
                    names.insert(row["name"].as_str()),
                    "access/alias name collision: {}",
                    row["name"]
                );
            }
            match kind {
                "access" => {
                    let binding = (row["protocol"].as_str(), row["endpoint"].as_str());
                    let previous = bindings.insert(row["credential"].as_str(), binding);
                    ensure!(
                        previous.is_none_or(|old| old == binding),
                        "custody handle has conflicting protocol/endpoint bindings"
                    );
                }
                "preference" => {
                    let c = &self.0[&Kind("candidate").reference(&row["candidate"])?];
                    let o = &self.0[&Kind("offering").reference(&c["offering"])?];
                    let a = &self.0[&Kind("access").reference(&o["access"])?];
                    ensure!(
                        scopes.insert((
                            a["provider"].as_str(),
                            o["family"].as_str(),
                            c["scenario"].as_str()
                        )),
                        "conflicting scenario preferences"
                    );
                }
                _ => (),
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct Estate {
    pub graph: Graph,
    ids: BTreeMap<Key, i64>,
}

impl Estate {
    pub fn snapshot(rows: Vec<Value>) -> Result<Self> {
        let mut records = BTreeMap::new();
        for mut row in rows {
            let r = rank(&row)?;
            let id = identifier(&mut row)?;
            ensure!(
                records.insert((r, id), row).is_none(),
                "duplicate native ID"
            );
        }
        let mut estate = Self::default();
        let mut names: BTreeMap<(usize, i64), Value> = BTreeMap::new();
        for ((r, id), mut row) in records {
            for &f in Kind(ORDER[r]).refs() {
                let target = ORDER.iter().position(|k| *k == f).unwrap();
                let local = row[f].as_i64().context("invalid native reference")?;
                row[f] = names
                    .get(&(target, local))
                    .context("dangling native reference")?
                    .clone();
            }
            let k = key(&row)?;
            let portable = if k.1.len() == 1 {
                json!(k.1[0])
            } else {
                json!(k.1)
            };
            ensure!(estate.ids.insert(k, id).is_none(), "ambiguous portable key");
            estate.graph.insert(row)?;
            names.insert((r, id), portable);
        }
        estate.graph.validate()?;
        Ok(estate)
    }

    pub fn plan(&mut self, wanted: Graph) -> Result<Vec<Value>> {
        wanted.validate()?;
        let mut create = Vec::new();
        for (k, row) in wanted.0 {
            if !self.ids.contains_key(&k) {
                create.push(row.clone());
            }
            self.graph.insert(row)?;
        }
        self.graph.validate()?;
        Ok(create)
    }

    pub fn native(&self, row: &Value) -> Result<Value> {
        let mut native = row.clone();
        for &f in Kind(ORDER[rank(row)?]).refs() {
            native[f] = json!(
                self.ids
                    .get(&Kind(f).reference(&row[f])?)
                    .context("unresolved local ID")?
            );
        }
        Ok(native)
    }

    pub fn created(&mut self, row: &Value, native: &Value, mut reply: Value) -> Result<()> {
        let id = identifier(&mut reply)?;
        ensure!(
            &reply == native,
            "unexpected catalog create result; recheck before retry"
        );
        self.ids.insert(key(row)?, id);
        Ok(())
    }
}

fn identifier(row: &mut Value) -> Result<i64> {
    row.as_object_mut()
        .and_then(|o| o.remove("id"))
        .and_then(|v| v.as_i64())
        .context("missing catalog record ID")
}

pub(super) fn label(row: &Value) -> Value {
    let (r, parts) = key(row).expect("only validated rows enter the plan");
    json!({"kind": ORDER[r], "key": parts})
}
