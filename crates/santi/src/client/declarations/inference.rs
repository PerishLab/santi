use anyhow::Result;

use super::super::Http;
use crate::cli::{ClientDefaults, Inference, Scope, Sources};

pub(in crate::client) async fn run(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    command: Inference,
) -> Result<()> {
    let route = match command {
        Inference::Sources(command) => return sources(http, base, defaults, command).await,
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

async fn sources(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    command: Sources,
) -> Result<()> {
    use super::super::http::urlencoding_encode as encode;
    use anyhow::Context;
    let url = format!("{base}/api/v1/inference/sources");
    let (method, path) = match command {
        Sources::List => return http.get(&url).await,
        Sources::Explain {
            provider,
            family,
            scenario,
        } => {
            return http
                .get(&format!(
                    "{base}/api/v1/inference/choices/{}/{}/{}",
                    encode(&provider),
                    encode(&family),
                    encode(&scenario)
                ))
                .await;
        }
        Sources::Create { file } => (reqwest::Method::POST, file),
        Sources::Update { file } => (reqwest::Method::PATCH, file),
        Sources::Delete { file } => (reqwest::Method::DELETE, file),
    };
    let custody = defaults
        .operator
        .as_deref()
        .context("configure the catalog operator token file in the client operator setting")?;
    let token = std::fs::read_to_string(custody).context("read catalog operator token file")?;
    let mut header = reqwest::header::HeaderValue::from_str(token.trim_end())
        .context("invalid catalog operator token")?;
    header.set_sensitive(true);
    let payload: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).context("read catalog JSON")?)
            .context("parse catalog JSON")?;
    let request = http
        .client
        .request(method.clone(), &url)
        .header("x-santi-catalog-token", header)
        .json(&payload);
    http.send(request, format!("{method} {url}")).await
}
