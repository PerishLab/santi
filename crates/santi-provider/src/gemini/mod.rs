use crate::{Cap, Headers, Metadata, Provider, Request, Streaming, Tool};
use async_trait::async_trait;
use reqwest::{Client, Url};
use serde_json::{Value, json};
use std::sync::Arc;
mod history;
mod stream;

pub struct Config {
    pub provider: String,
    pub model: String,
    pub key: String,
    pub url: String,
    pub effort: Option<String>,
    pub ceiling: Option<u32>,
    pub bytes: usize,
    pub maintenance: usize,
    pub headers: Vec<String>,
    pub outgoing: Headers,
}

pub struct Gemini {
    config: Config,
    client: Client,
    root: Url,
}

impl Gemini {
    pub fn new(config: Config) -> Result<Self, String> {
        let root = Url::parse(&config.url).map_err(|e| e.to_string())?;
        let origin = matches!(root.scheme(), "http" | "https") && root.host().is_some();
        let credentials = !root.username().is_empty() || root.password().is_some();
        let suffix = root.query().is_some() || root.fragment().is_some();
        if !origin || credentials || suffix {
            return Err("Gemini base_url must be an HTTP origin or path without credentials, query or fragment".into());
        }
        Ok(Self {
            config,
            client: Client::new(),
            root,
        })
    }
    fn body(&self, req: &Request, complete: bool) -> Result<Value, String> {
        let contents = history::contents(&self.config, req, complete)?;
        let mut body = json!({"contents": contents});
        if let Some(s) = req.instructions.as_ref().filter(|s| !s.trim().is_empty()) {
            body["systemInstruction"] = json!({"parts":[{"text":s}]});
        }
        if let Some(tools) = req.tools.as_ref().filter(|t| !t.is_empty()) {
            let declarations = tools
                .iter()
                .map(|Tool::Function(f)| {
                    json!({
                        "name": f.name,
                        "description": f.description,
                        "parametersJsonSchema": f.parameters
                    })
                })
                .collect::<Vec<_>>();
            body["tools"] = json!([{"functionDeclarations":declarations}]);
        }
        if let Some(n) = self.config.ceiling {
            body["generationConfig"]["maxOutputTokens"] = json!(n);
        }
        if let Some(effort) = &self.config.effort {
            body["generationConfig"]["thinkingConfig"]["thinkingLevel"] =
                json!(effort.to_ascii_uppercase());
        }
        Ok(body)
    }
}

#[async_trait]
impl Provider for Gemini {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from(self.config.provider.as_str()),
            model: self.config.model.clone(),
            budget: Some(Cap {
                bytes: self.config.bytes,
                maintenance: self.config.maintenance,
                source: "provider_config".into(),
            }),
        }
    }
    fn prepare(&self, req: Request) -> Result<Request, String> {
        Ok(req)
    }
    fn validate(&self, req: &Request) -> Result<(), String> {
        history::contents(&self.config, req, false).map(|_| ())
    }
    fn estimate(&self, req: &Request) -> Option<santi_model::budget::Estimate> {
        let body = self.body(req, false).ok()?;
        let bytes = |v: &Value| serde_json::to_vec(v).ok().map(|b| b.len() as i64);
        Some(santi_model::budget::Estimate {
            estimator: "gemini_generate_content_json_bytes_v1".into(),
            items: body["contents"].as_array()?.len() as i64,
            input: bytes(&body["contents"])?,
            instructions: body.get("systemInstruction").and_then(bytes).unwrap_or(0),
            tools: body.get("tools").and_then(bytes).unwrap_or(0),
            total: bytes(&body)?,
        })
    }
    async fn stream(&self, req: Request) -> Result<Streaming, String> {
        let body = self.body(&req, true)?;
        let mut url = self.root.clone();
        url.path_segments_mut()
            .map_err(|_| "invalid Gemini URL")?
            .pop_if_empty()
            .extend(["models", &format!("{}:streamGenerateContent", req.model)]);
        url.query_pairs_mut().append_pair("alt", "sse");
        let sent = std::time::Instant::now();
        let resp = self
            .client
            .post(url)
            .bearer_auth(&self.config.key)
            .headers(self.config.outgoing.render(req.strand.as_deref())?)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                use std::error::Error;
                let cause = error.source().map(ToString::to_string).unwrap_or_default();
                format!("Gemini transport: {error} {cause}")
            })?;
        if !resp.status().is_success() {
            return Err(format!(
                "Gemini stream request failed: HTTP {}",
                resp.status()
            ));
        }
        let receipt = crate::receipt::received(&resp, &self.config.headers, sent);
        let state = santi_model::thinking::Continuation {
            provider: self.config.provider.clone(),
            model: req.model,
            endpoint: self.config.url.clone(),
            content: None,
            calls: 0,
            parts: Some(Vec::new()),
        };
        Ok(Box::pin(stream::frames(
            receipt,
            resp.bytes_stream(),
            state,
        )))
    }
}

#[cfg(test)]
mod tests;
