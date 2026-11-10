use std::io::Write as _;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;

use crate::cli::{ClientDefaults, Command, EffectCommand, Job, Turn, WatchFormat, Webhook};
use crate::text::source::read_summary_file;
use crate::watch::{next_sse_frame, render_watch_event};

mod create;
mod declarations;
mod jobs;
mod send;
pub(crate) use send::guidance;
mod strand;
pub mod tui;

pub(crate) use send::{Failed, Proof, pending, prove, uncertain};
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

pub(crate) struct Run<'a> {
    pub base_url: &'a str,
    pub bearer: Option<&'a str>,
    pub defaults: &'a ClientDefaults,
    pub explicit: bool,
}

#[locus::trace(with = plumb::observation::view())]
pub(crate) async fn run(run: Run<'_>, command: Command) -> Result<()> {
    let Run {
        base_url,
        bearer,
        defaults,
        explicit,
    } = run;
    let client = build_client(bearer)?;
    let http = Http { client: &client };
    let base = base_url.trim_end_matches('/').to_string();
    match command {
        Command::Skill(_) => {
            anyhow::bail!("skill commands must dispatch before HTTP configuration")
        }
        Command::Health => http.get(&format!("{base}/api/v1/health")).await,
        Command::Schema => http.get(&format!("{base}/api/v1/openapi.json")).await,
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
            before,
        } => {
            http.get(&http::page(
                format!("{base}/api/v1/errors/{scope_kind}/{scope_id}"),
                limit,
                before.as_deref(),
            )?)
            .await
        }
        Command::Soul(args) => declarations::materials::soul(&http, &base, defaults, args).await,
        Command::Incident { id } => http.get(&format!("{base}/api/v1/incidents/{id}")).await,
        Command::Reply(args) => {
            declarations::reply::Reply {
                http: &http,
                base: &base,
                defaults,
            }
            .run(args)
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
        Command::Inference(command) => {
            declarations::inference::run(&http, &base, defaults, command).await
        }
        Command::Compact(command) => {
            declarations::compact::run(&http, &base, defaults, command).await
        }
        Command::Job(Job::Create {
            description,
            command,
            cwd,
            timeout_seconds,
            output_limit_bytes,
            remind_every_seconds,
            notify_on_terminal,
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
                    "notify_on_terminal": notify_on_terminal,
                }),
            )
            .await
        }
        Command::Query { dsl } => jobs::raw(&http, &base, defaults, dsl).await,
        Command::Job(command @ Job::Query { .. }) => {
            jobs::query(
                jobs::Query {
                    http: &http,
                    base: &base,
                    defaults,
                    explicit,
                },
                command,
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
        Command::Alarm(command) => declarations::alarm::run(&http, &base, defaults, command).await,
        Command::Turn(Turn::Show { id }) => http.get(&format!("{base}/api/v1/turns/{id}")).await,
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
        Command::Environment(command) => {
            declarations::environment::run(&http, &base, command).await
        }
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
