use async_trait::async_trait;
use futures_core::Stream;
use reqwest::Client;
use std::sync::Arc;

use crate::{Metadata, Provider, Request, Streaming};

#[derive(Debug, Clone)]
pub struct Config {
    pub provider: String,
    pub key: String,
    pub model: String,
    pub url: String,
    pub thinking: Option<String>,
    pub replay: Replay,
    pub effort: Option<String>,
    pub ceiling: Option<u32>,
    pub bytes: Option<usize>,
    pub maintenance: Option<usize>,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Replay {
    #[default]
    Stateless,
    History,
}

#[derive(Debug, Clone)]
pub struct Chat {
    config: Config,
    client: Client,
}

impl Chat {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }
}

#[async_trait]
impl Provider for Chat {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from(self.config.provider.clone()),
            model: self.config.model.clone(),
            budget: self.config.bytes.map(|bytes| crate::Cap {
                bytes,
                maintenance: self
                    .config
                    .maintenance
                    .unwrap_or(bytes.saturating_add(bytes / 2)),
                source: "provider_config".to_string(),
            }),
        }
    }

    fn prepare(&self, request: Request) -> Result<Request, String> {
        replay::prepare(&self.config, request)
    }

    fn validate(&self, request: &Request) -> Result<(), String> {
        replay::validate(&self.config, request, false)
    }

    fn estimate(&self, request: &Request) -> Option<santi_model::budget::Estimate> {
        let body = body(&self.config, request.clone());
        let input = serde_json::to_vec(&body["messages"]).ok()?.len() as i64;
        let tools = body
            .get("tools")
            .map(serde_json::to_vec)
            .transpose()
            .ok()?
            .map_or(0, |bytes| bytes.len()) as i64;
        Some(santi_model::budget::Estimate {
            estimator: "chat_completions_json_bytes_v1".into(),
            items: body["messages"].as_array()?.len() as i64,
            input,
            instructions: 0,
            tools,
            total: serde_json::to_vec(&body).ok()?.len() as i64,
        })
    }

    async fn stream(&self, request: Request) -> Result<Streaming, String> {
        let request = self.prepare(request)?;
        replay::validate(&self.config, &request, true)?;
        let continuation = santi_model::thinking::Continuation {
            calls: 0,
            provider: self.config.provider.clone(),
            model: request.model.clone(),
            endpoint: self.config.url.clone(),
            content: None,
        };
        let sent = std::time::Instant::now();
        let response = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.config.url.trim_end_matches('/')
            ))
            .bearer_auth(&self.config.key)
            .json(&body(&self.config, request))
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "{} chat completions request failed: {status} {body}",
                self.config.provider
            ));
        }
        let receipt = crate::receipt::received(&response, &self.config.headers, sent);
        Ok(Box::pin(frames(
            receipt,
            response.bytes_stream(),
            continuation,
        )))
    }
}

mod body;
mod message;
mod replay;
mod stream;
use body::*;
use stream::*;

#[cfg(test)]
mod tests;
