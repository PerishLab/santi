use std::io::Write as _;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;

use crate::cli::wake::Wake;
use crate::cli::{
    ClientDefaults, Command, CompactCommand, EffectCommand, Job, Turn, WatchFormat, Webhook,
};
use crate::text::source::read_summary_file;
use crate::watch::{next_sse_frame, render_watch_event};

mod create;
mod environment;
mod jobs;
mod send;
mod strand;
pub mod tui;

pub(crate) use send::{Proof, prove, uncertain, unsettled};
pub use send::{Request, Target, emit, send};

const TIMEOUT: Duration = Duration::from_secs(30);

struct Capsule<'a> {
    first: Option<String>,
    last: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    summary: Option<String>,
    file: Option<String>,
    source: String,
    reason: String,
    risk: String,
    queryability: String,
    preview: bool,
    soul: Option<&'a str>,
}

pub(crate) async fn run(
    base_url: &str,
    bearer: Option<&str>,
    defaults: &ClientDefaults,
    command: Command,
) -> Result<()> {
    let client = build_client(bearer)?;
    let http = Http { client: &client };
    let base = base_url.trim_end_matches('/').to_string();
    match command {
        Command::Health => http.get(&format!("{base}/api/v1/health")).await,
        Command::Tui { memory } => {
            tui::run(tui::Request {
                client: &client,
                base: &base,
                defaults,
                bearer,
                memory,
            })
            .await
        }
        Command::Errors {
            scope_kind,
            scope_id,
            limit,
        } => {
            http.get(&format!(
                "{base}/api/v1/errors/{scope_kind}/{scope_id}?limit={limit}"
            ))
            .await
        }
        Command::Receipt { inbox } => http.get(&format!("{base}/api/v1/receipts/{inbox}")).await,
        Command::Effect(EffectCommand::Query { effect }) => {
            http.get(&format!("{base}/api/v1/effects/{effect}")).await
        }
        Command::Effect(EffectCommand::Resolve {
            effect,
            outcome,
            evidence,
        }) => {
            http.post(
                &format!("{base}/api/v1/effects/{effect}/resolve"),
                Some(serde_json::json!({
                    "outcome": outcome.as_api_str(),
                    "evidence": evidence,
                })),
            )
            .await
        }
        Command::Strand(command) => strand::run(&http, &base, defaults, command).await,
        Command::Compact(CompactCommand::Exec {
            first,
            last,
            summary,
            summary_file,
            absorb,
        }) => {
            let strand = defaults.resolve_strand(None)?;
            let summary = match summary_file {
                Some(path) => read_summary_file(&path)?,
                None => summary.expect("clap requires summary or summary_file"),
            };
            let mut body = serde_json::json!({ "summary": summary });
            if !absorb.is_empty() {
                body["absorb"] = serde_json::Value::from(absorb);
            }
            if let (Some(first), Some(last)) = (first, last) {
                body["first"] = serde_json::Value::from(first);
                body["last"] = serde_json::Value::from(last);
            }
            if let Some(soul) = defaults.soul() {
                body["soul"] = serde_json::Value::from(soul);
            }
            http.post(
                &format!("{base}/api/v1/strands/{strand}/compact"),
                Some(body),
            )
            .await
        }
        Command::Compact(CompactCommand::Capsule {
            first,
            last,
            from,
            to,
            summary,
            summary_file,
            source,
            reason,
            risk,
            queryability,
            dry,
        }) => {
            let body = compact_capsule_body(Capsule {
                first,
                last,
                from,
                to,
                summary,
                file: summary_file,
                source,
                reason,
                risk,
                queryability,
                preview: dry,
                soul: defaults.soul(),
            })?;
            let strand = defaults.resolve_strand(None)?;
            http.post(
                &format!("{base}/api/v1/strands/{strand}/compact"),
                Some(body),
            )
            .await
        }
        Command::Compact(CompactCommand::Query {
            compact,
            keyword,
            page_index,
            page_size,
        }) => {
            let mut url = format!(
                "{base}/api/v1/compacts/{compact}?page_index={page_index}&page_size={page_size}"
            );
            if let Some(keyword) = keyword.filter(|k| !k.is_empty()) {
                url.push_str(&format!("&keyword={}", urlencoding_encode(&keyword)));
            }
            http.get(&url).await
        }
        Command::Job(Job::Create {
            description,
            command,
            cwd,
            timeout_seconds,
            output_limit_bytes,
            remind_every_seconds,
        }) => {
            let capability = crate::config::env("SANTI_JOB_CREATE_CAPABILITY").ok_or_else(|| {
                anyhow::anyhow!(
                    "no job create capability: run this command from a Santi runtime shell invocation"
                )
            })?;
            http.spawn(
                &format!("{base}/api/v1/jobs"),
                &capability,
                serde_json::json!({
                    "description": description,
                    "command": command,
                    "cwd": cwd,
                    "timeout_seconds": timeout_seconds,
                    "output_limit_bytes": output_limit_bytes,
                    "remind_every_seconds": remind_every_seconds,
                }),
            )
            .await
        }
        Command::Job(Job::List) => {
            http.owned(&format!("{base}/api/v1/jobs"), defaults.require()?)
                .await
        }
        Command::Job(Job::Get { id }) => {
            http.owned(&format!("{base}/api/v1/jobs/{id}"), defaults.require()?)
                .await
        }
        Command::Job(Job::Cancel { id }) => {
            http.act(
                &format!("{base}/api/v1/jobs/{id}/cancel"),
                defaults.require()?,
            )
            .await
        }
        Command::Job(Job::Logs {
            id,
            stream,
            cursor,
            limit,
        }) => {
            let cursor = urlencoding_encode(&cursor);
            http.owned(
                &format!(
                    "{base}/api/v1/jobs/{id}/logs?stream={}&cursor={cursor}&limit={limit}",
                    stream.wire()
                ),
                defaults.require()?,
            )
            .await
        }
        Command::Job(Job::Ack { id }) => {
            http.act(&format!("{base}/api/v1/jobs/{id}/ack"), defaults.require()?)
                .await
        }
        Command::Wake(Wake::Status) => {
            let soul = defaults.require()?;
            http.get(&format!("{base}/api/v1/souls/{soul}/wake")).await
        }
        Command::Wake(Wake::Enable) => {
            let soul = defaults.require()?;
            http.post(
                &format!("{base}/api/v1/souls/{soul}/wake"),
                Some(serde_json::json!({"action": "enable"})),
            )
            .await
        }
        Command::Wake(Wake::Disable) => {
            let soul = defaults.require()?;
            http.post(
                &format!("{base}/api/v1/souls/{soul}/wake"),
                Some(serde_json::json!({"action": "disable"})),
            )
            .await
        }
        Command::Turn(Turn::Stop { id }) => {
            http.post(&format!("{base}/api/v1/turns/{id}/stop"), None)
                .await
        }
        Command::Turn(Turn::Active) => {
            let soul = defaults.require()?;
            let mut url = format!("{base}/api/v1/souls/{soul}/active-turns");
            if let Some(strand) = defaults
                .strand
                .as_deref()
                .map(str::trim)
                .filter(|strand| !strand.is_empty())
            {
                url.push_str(&format!("?exclude={}", urlencoding_encode(strand)));
            }
            http.get(&url).await
        }
        Command::Environment(command) => environment::run(&http, &base, command).await,
        Command::Webhook(Webhook::List) => http.get(&format!("{base}/api/v1/webhooks")).await,
        Command::Webhook(Webhook::Ensure {
            name,
            adaptor,
            soul,
            strategy,
            credential,
        }) => {
            http.post(
                &format!("{base}/api/v1/webhooks"),
                Some(serde_json::json!({
                    "name": name,
                    "adaptor": adaptor,
                    "soul": soul,
                    "strategy": strategy.encode(),
                    "credential": credential,
                })),
            )
            .await
        }
    }
}

mod http;
use http::*;
