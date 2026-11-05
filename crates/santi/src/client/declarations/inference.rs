use anyhow::Result;

use super::super::Http;
use crate::cli::{ClientDefaults, Inference, Scope};

pub(in crate::client) async fn run(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    command: Inference,
) -> Result<()> {
    let route = match command {
        Inference::Soul(scope) => Route {
            path: "souls",
            command: scope,
        },
        Inference::Strand(scope) => Route {
            path: "strands",
            command: scope,
        },
    };
    target(http, base, defaults, route).await
}

struct Route<'a> {
    path: &'a str,
    command: Scope,
}

async fn target(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    route: Route<'_>,
) -> Result<()> {
    let Route { path, command } = route;
    match command {
        Scope::Get { id } => {
            let id = resolve(defaults, id, path)?;
            http.get(&format!("{base}/api/v1/{path}/{id}/inference"))
                .await
        }
        Scope::Set(edit) => {
            let id = resolve(defaults, edit.id.clone(), path)?;
            let patch = edit.patch(&defaults.held)?;
            http.patch(
                &format!("{base}/api/v1/{path}/{id}/inference"),
                serde_json::to_value(patch)?,
            )
            .await
        }
    }
}

fn resolve(defaults: &ClientDefaults, id: Option<String>, scope: &str) -> Result<String> {
    if scope == "strands" {
        return defaults.resolve_strand(id);
    }
    match id.map(|id| id.trim().to_string()) {
        Some(id) if !id.is_empty() => Ok(id),
        _ => defaults.require().map(str::to_string),
    }
}
