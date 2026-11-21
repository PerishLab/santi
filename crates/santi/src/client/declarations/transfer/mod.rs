mod graph;
mod kind;
mod legacy;
#[cfg(test)]
mod tests;

use super::super::Http;
use super::inference::operator;
use anyhow::{Context, Result, anyhow, ensure};
use graph::{Estate, Graph};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    version: u32,
    resources: Vec<Value>,
}

async fn snapshot(http: &Http<'_>, url: &str) -> Result<Estate> {
    let response = http
        .client
        .get(url)
        .timeout(crate::client::TIMEOUT)
        .send()
        .await?
        .error_for_status()?;
    Estate::snapshot(response.json().await.context("decode catalog snapshot")?)
}

pub(super) async fn export(http: &Http<'_>, url: &str) -> Result<()> {
    let estate = snapshot(http, url).await?;
    let archive = Archive {
        version: 1,
        resources: estate.graph.rows().cloned().collect(),
    };
    println!("{}", serde_json::to_string_pretty(&archive)?);
    Ok(())
}

fn portable(source: &str) -> Result<Graph> {
    let archive: Archive = serde_json::from_str(source).map_err(|_| {
        anyhow!("expected portable catalog JSON; for legacy TOML provide --mapping")
    })?;
    ensure!(archive.version == 1, "unsupported portable catalog version");
    let mut graph = Graph::default();
    for row in archive.resources {
        graph.insert(row)?;
    }
    graph.validate()?;
    Ok(graph)
}

pub(super) async fn import(
    http: &Http<'_>,
    url: &str,
    input: crate::cli::Import,
    custody: Option<&Path>,
) -> Result<()> {
    let source = std::fs::read_to_string(input.file).context("read import file")?;
    let (desired, coverage) = match input.mapping {
        Some(path) => legacy::convert(
            &source,
            &std::fs::read_to_string(path).context("read mapping")?,
        )?,
        None => (portable(&source)?, Value::Null),
    };
    let total = desired.rows().count();
    let mut estate = snapshot(http, url).await?;
    let creates = estate.plan(desired)?;
    if input.apply && !creates.is_empty() {
        let header = operator(custody)?;
        for (i, row) in creates.iter().enumerate() {
            let native = estate.native(row)?;
            let response = http
                .client
                .post(url)
                .timeout(crate::client::TIMEOUT)
                .header("x-santi-catalog-token", header.clone())
                .json(&native)
                .send()
                .await
                .with_context(|| format!("{i} creates confirmed; run dry-run before retry"))?;
            ensure!(
                response.status().is_success(),
                "create {} refused ({}), {i} confirmed; run dry-run before retry",
                row["kind"],
                response.status()
            );
            estate.created(
                row,
                &native,
                response
                    .json()
                    .await
                    .context("invalid create receipt; run dry-run before retry")?,
            )?;
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"mode":if input.apply {"apply"} else {"dry-run"}, "matched":total-creates.len(), "create":creates.len(), "coverage":coverage,
                "resources_to_create": creates.iter().map(graph::label).collect::<Vec<_>>()
            })
        )?
    );
    Ok(())
}
