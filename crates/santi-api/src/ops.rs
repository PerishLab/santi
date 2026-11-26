use std::collections::HashMap;
use std::fs;

use serde::Serialize;

use crate::config::Layout;
use crate::runtime::{self, Runtime};

#[derive(Debug, Clone, Serialize)]
pub struct Doctor {
    pub database: String,
    #[serde(rename = "database_exists")]
    pub exists: bool,
    #[serde(rename = "estate_bound")]
    pub bound: bool,
    #[serde(rename = "estate_ready")]
    pub ready: bool,
    #[serde(rename = "estate_error")]
    pub error: Option<String>,
    pub genesis: String,
    #[serde(rename = "memory_path")]
    pub path: String,
    #[serde(rename = "memory_present")]
    pub present: bool,
    #[serde(rename = "memory_readable")]
    pub readable: bool,
    #[serde(rename = "memory_bytes")]
    pub bytes: u64,
    pub provider: Option<Provider>,
    pub ok: bool,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Provider {
    pub scenario: Option<String>,
    pub kind: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub bytes: Option<usize>,
    pub maintenance: Option<usize>,
    pub source: Option<String>,
    pub ok: bool,
    pub error: Option<String>,
}

pub async fn doctor() -> Result<Doctor, String> {
    let held = runtime::held();
    held.paths.configured(held).await
}

#[derive(Debug, Clone, Serialize)]
pub struct Seed {
    pub strand: String,
    pub accepted: bool,
    pub error: Option<santi_core::Fault>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Row {
    #[serde(rename = "created_at")]
    pub created: String,
    pub status: String,
    #[serde(rename = "strand_id")]
    pub strand: String,
    #[serde(rename = "turn_id")]
    pub turn: String,
    #[serde(rename = "tool_name")]
    pub tool: String,
    pub arguments: serde_json::Value,
    pub output: Option<serde_json::Value>,
    #[serde(rename = "error_text")]
    pub error: Option<String>,
}

pub struct Audit<'a> {
    pub strand: Option<&'a str>,
    pub turn: Option<&'a str>,
    pub failed: bool,
    pub limit: usize,
    pub after: Option<&'a str>,
}

pub async fn seed(strand: &str, text: &str) -> Result<Seed, String> {
    runtime::held().paths.seed(strand, text).await
}

async fn enqueue(store: &santi_core::Store, strand: &str, text: &str) -> Result<Seed, String> {
    let inbox = santi_core::tag("inbox");
    let content = santi_core::message::Content::text(text);
    let source = santi_core::ingest::Source::new("offline_inbox_seed");
    store
        .accept(
            santi_core::Arrival {
                tag: &inbox,
                strand,
                kind: santi_core::message::Kind::System,
                content: &content,
                source: Some(&source),
                created: &santi_core::now(),
            },
            500,
        )
        .await?;
    Ok(Seed {
        strand: strand.to_string(),
        accepted: true,
        error: None,
    })
}

impl Layout {
    pub async fn audit(&self, query: Audit<'_>) -> Result<Vec<Row>, String> {
        let Audit {
            strand,
            turn,
            failed,
            limit,
            after,
        } = query;
        let store = santi_core::Store::open(&self.database).await?;
        let strands = match strand {
            Some(tag) => vec![
                store
                    .strand(tag)
                    .await?
                    .ok_or_else(|| format!("unknown strand: {tag}"))?,
            ],
            None => store.strands().await?,
        };
        let mut rows = Vec::new();
        for strand in strands {
            let replies = store
                .results(&strand.id)
                .await?
                .into_iter()
                .map(|reply| (reply.call.clone(), reply))
                .collect::<HashMap<_, _>>();
            for call in store.calls(&strand.id).await? {
                if turn.is_some_and(|turn| turn != call.turn)
                    || after.is_some_and(|after| call.created.as_str() <= after)
                {
                    continue;
                }
                let held = store
                    .turn(&call.turn)
                    .await?
                    .ok_or_else(|| format!("tool call turn {} missing", call.turn))?;
                let reply = replies.get(&call.id);
                let errored = held.status == santi_core::turn::Status::Failed
                    || reply.and_then(|reply| reply.error.as_ref()).is_some()
                    || reply
                        .and_then(|reply| reply.output.as_ref())
                        .and_then(|output| output.get("exit_code"))
                        .and_then(serde_json::Value::as_i64)
                        .is_some_and(|exit| exit != 0);
                if failed && !errored {
                    continue;
                }
                rows.push(Row {
                    created: call.created,
                    status: match held.status {
                        santi_core::turn::Status::Running => "running",
                        santi_core::turn::Status::Completed => "completed",
                        santi_core::turn::Status::Failed => "failed",
                    }
                    .to_string(),
                    strand: strand.id.clone(),
                    turn: call.turn,
                    tool: call.tool,
                    arguments: call.arguments,
                    output: reply.and_then(|reply| reply.output.clone()),
                    error: reply.and_then(|reply| reply.error.clone()),
                });
            }
        }
        rows.sort_by(|left, right| {
            left.created
                .cmp(&right.created)
                .then_with(|| left.turn.cmp(&right.turn))
        });
        if rows.len() > limit {
            rows.drain(..rows.len() - limit);
        }
        Ok(rows)
    }

    pub async fn doctor(&self) -> Result<Doctor, String> {
        self.diagnose(None).await
    }

    pub async fn configured(&self, held: &Runtime) -> Result<Doctor, String> {
        let mut provider = Provider {
            scenario: held.inference.scenario.clone(),
            effort: held.inference.effort.clone(),
            ..Default::default()
        };
        let resolved = async {
            use crate::provider::Catalog;
            use santi_core::inference::Origin;
            let rows = if self.database.exists() {
                santi_core::Store::open(&self.database)
                    .await?
                    .sources()
                    .await?
            } else {
                Vec::new()
            };
            let catalog = Catalog {
                custody: &held.custody,
            };
            let layers = [(&held.inference, Origin::Service)];
            let (route, model) = Catalog::offered(&layers, &rows)?;
            provider.kind = Some(route.protocol.clone());
            provider.model = Some(model.into());
            let selection = catalog.admit(&layers, route, model)?;
            catalog.configured(&selection)
        }
        .await;
        match resolved {
            Ok(config) => {
                provider.effort = config.effort().map(str::to_owned);
                provider.bytes = Some(config.bytes());
                provider.maintenance = Some(config.maintenance());
                provider.source = Some("service_selection".into());
                provider.ok = true;
            }
            Err(error) => provider.error = Some(error),
        }
        self.diagnose(Some(provider)).await
    }

    async fn diagnose(&self, provider: Option<Provider>) -> Result<Doctor, String> {
        let exists = self.database.exists();
        let (bound, ready, error) = if exists {
            match santi_core::Store::open(&self.database).await {
                Ok(store) => match store.soul(santi_core::GENESIS).await {
                    Ok(Some(_)) => (true, true, None),
                    Ok(None) => (true, false, Some("genesis soul is missing".to_string())),
                    Err(error) => (true, false, Some(error)),
                },
                Err(error) => (false, false, Some(error)),
            }
        } else {
            (false, false, Some("estate database is missing".to_string()))
        };
        let path = self
            .runtime
            .join("souls")
            .join(santi_core::GENESIS)
            .join("memory")
            .join(santi_core::MEMORY);
        let present = path.exists();
        let (readable, bytes) = match fs::read(&path) {
            Ok(bytes) => (true, bytes.len() as u64),
            Err(_) => (false, 0),
        };
        let memorized = !present || readable;
        let configured = provider.as_ref().is_none_or(|provider| provider.ok);
        Ok(Doctor {
            database: self.database.display().to_string(),
            exists,
            bound,
            ready,
            error,
            genesis: santi_core::GENESIS.to_string(),
            path: path.display().to_string(),
            present,
            readable,
            bytes,
            provider,
            ok: ready && memorized && configured,
        })
    }

    pub async fn seed(&self, strand: &str, text: &str) -> Result<Seed, String> {
        let store = santi_core::Store::open(&self.database).await?;
        if store.strand(strand).await?.is_none() {
            return Err(format!("unknown strand: {strand}"));
        }
        enqueue(&store, strand, text).await
    }

    pub async fn labeled(&self, soul: &str, label: &str, text: &str) -> Result<Seed, String> {
        let store = santi_core::Store::open(&self.database).await?;
        let strand = store.labeled(soul, label, &santi_core::now()).await?;
        enqueue(&store, &strand.id, text).await
    }
}
