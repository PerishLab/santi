use anyhow::Result;

use super::{Http, Request, Target, send, strand_send_body};
use crate::cli::{ClientDefaults, StrandCommand, split_send_args};

pub(super) async fn run(
    http: &Http<'_>,
    base: &str,
    defaults: &ClientDefaults,
    command: StrandCommand,
) -> Result<()> {
    match command {
        StrandCommand::Create => http.create_strand(base, defaults.soul()).await,
        StrandCommand::List => http.get(&format!("{base}/api/v1/strands")).await,
        StrandCommand::Get { id } => {
            let id = defaults.resolve_strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}")).await
        }
        StrandCommand::Messages { id } => {
            let id = defaults.resolve_strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/messages"))
                .await
        }
        StrandCommand::Runtime { id } => {
            let id = defaults.resolve_strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/runtime"))
                .await
        }
        StrandCommand::ExecutionTail { id } => {
            let id = defaults.resolve_strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/execution-tail"))
                .await
        }
        StrandCommand::Budget { id } => {
            let id = defaults.resolve_strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/budget"))
                .await
        }
        StrandCommand::Errors { id, limit } => {
            let id = defaults.resolve_strand(id)?;
            http.get(&format!("{base}/api/v1/strands/{id}/errors?limit={limit}"))
                .await
        }
        StrandCommand::Fork { id } => {
            let id = defaults.resolve_strand(id)?;
            http.post(&format!("{base}/api/v1/strands/{id}/fork"), None)
                .await
        }
        StrandCommand::Drive { id } => {
            let id = defaults.resolve_strand(id)?;
            http.post(&format!("{base}/api/v1/strands/{id}/drive"), None)
                .await
        }
        StrandCommand::Send {
            args,
            watch,
            watch_format,
        } => {
            let (id, text) = split_send_args(args, defaults)?;
            let content = strand_send_body(text, defaults.soul());
            send(Request {
                target: Target::new(http.client, base, &id, watch_format),
                body: content,
                watch,
            })
            .await
        }
        StrandCommand::Events { id, format } => {
            let id = defaults.resolve_strand(id)?;
            http.follow(&format!("{base}/api/v1/strands/{id}/events"), format)
                .await
        }
    }
}
