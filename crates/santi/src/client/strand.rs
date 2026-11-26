use anyhow::Result;

use super::{Http, Request, Target, payload, send};
use crate::cli::{Defaults, Strand, split};

pub(super) async fn run(
    http: &Http<'_>,
    base: &str,
    defaults: &Defaults,
    command: Strand,
) -> Result<()> {
    match command {
        Strand::Material(args) => {
            super::declarations::materials::Seat::new(http, base, defaults)
                .run("strands", args)
                .await
        }
        Strand::Create(delta) => {
            http.strand(base, defaults.soul(), delta.body(&defaults.held)?)
                .await
        }
        Strand::End { id } => http.delete(&format!("{base}/api/v1/strands/{id}")).await,
        Strand::List => http.get(&format!("{base}/api/v1/strands")).await,
        Strand::Get { id } => {
            let id = defaults.strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}")).await
        }
        Strand::Messages { id } => {
            let id = defaults.strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/messages"))
                .await
        }
        Strand::Runtime { id } => {
            let id = defaults.strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/runtime"))
                .await
        }
        Strand::Activity { id } => {
            let id = defaults.strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/activity"))
                .await
        }
        Strand::Tail { id } => {
            let id = defaults.strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/execution-tail"))
                .await
        }
        Strand::Budget { id } => {
            let id = defaults.strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/budget"))
                .await
        }
        Strand::Errors { id, limit, before } => {
            let id = defaults.strand(id)?;
            http.get(&super::http::page(
                format!("{base}/api/v1/strands/{id}/errors"),
                limit,
                before.as_deref(),
            )?)
            .await
        }
        Strand::Fork { id, at, delta } => {
            let id = defaults.strand(id)?;
            let body = positioned(delta.body(&defaults.held)?, at);
            http.post(&format!("{base}/api/v1/strands/{id}/fork"), body)
                .await
        }
        Strand::Drive { id } => {
            let id = defaults.strand(id)?;
            http.post(&format!("{base}/api/v1/strands/{id}/drive"), None)
                .await
        }
        Strand::Send {
            args,
            watch,
            format,
        } => {
            let (id, text) = split(args, defaults)?;
            let content = payload(text, defaults.soul());
            send(Request {
                target: Target::new(http.client, base, &id, format),
                body: content,
                watch,
            })
            .await
        }
        Strand::Events { id, format } => {
            let id = defaults.strand(id)?;
            http.follow(&format!("{base}/api/v1/strands/{id}/events"), format)
                .await
        }
    }
}

fn positioned(mut body: Option<serde_json::Value>, at: Option<i64>) -> Option<serde_json::Value> {
    if let Some(at) = at {
        body.get_or_insert_with(|| serde_json::json!({}))["at"] = at.into();
    }
    body
}
