mod fields;
mod secrecy;
use super::*;
use fields::{capacity, optional, positive, required};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    Responses(Responses),
    Completions(Completions),
    Gemini(Gemini),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Responses {
    pub key: String,
    pub model: String,
    pub endpoint: String,
    pub effort: Option<String>,
    pub summary: Option<String>,
    pub tokens: Option<u32>,
    pub bytes: usize,
    pub maintenance: usize,
    pub attempts: Option<u32>,
    pub headers: Vec<String>,
    pub outgoing: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gemini {
    pub provider: String,
    pub key: String,
    pub model: String,
    pub endpoint: String,
    pub effort: Option<String>,
    pub tokens: Option<u32>,
    pub bytes: usize,
    pub maintenance: usize,
    pub headers: Vec<String>,
    pub outgoing: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completions {
    pub provider: String,
    pub key: String,
    pub model: String,
    pub endpoint: String,
    pub thinking: Option<String>,
    pub replay: santi_provider::chat::completions::Replay,
    pub effort: Option<String>,
    pub tokens: Option<u32>,
    pub bytes: usize,
    pub maintenance: usize,
    pub headers: Vec<String>,
    pub outgoing: std::collections::BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Profile {
    #[serde(rename = "openai_responses")]
    Responses {
        #[serde(default)]
        seal: Option<String>,
        #[serde(default, rename = "api_key")]
        key: Option<String>,
        #[serde(default, rename = "base_url")]
        endpoint: Option<String>,
        #[serde(default, rename = "reasoning_effort")]
        effort: Option<String>,
        #[serde(default, alias = "reasoning_summary")]
        summary: Option<String>,
        #[serde(default, rename = "max_output_tokens")]
        tokens: Option<u32>,
        #[serde(default, alias = "input_budget_bytes")]
        bytes: Option<usize>,
        #[serde(default, rename = "maintenance_bytes")]
        maintenance: Option<usize>,
        #[serde(default)]
        attempts: Option<u32>,
        #[serde(default)]
        headers: Option<Vec<String>>,
        #[serde(default, rename = "request_headers")]
        outgoing: std::collections::BTreeMap<String, String>,
    },
    #[serde(rename = "gemini_generate_content")]
    Gemini {
        #[serde(default)]
        seal: Option<String>,
        #[serde(default)]
        identity: Option<String>,
        #[serde(default, rename = "api_key")]
        key: Option<String>,
        #[serde(default, rename = "base_url")]
        endpoint: Option<String>,
        #[serde(default, rename = "reasoning_effort")]
        effort: Option<String>,
        #[serde(default, rename = "max_tokens")]
        tokens: Option<u32>,
        #[serde(default, alias = "input_budget_bytes")]
        bytes: Option<usize>,
        #[serde(default, rename = "maintenance_bytes")]
        maintenance: Option<usize>,
        #[serde(default)]
        headers: Option<Vec<String>>,
        #[serde(default, rename = "request_headers")]
        outgoing: std::collections::BTreeMap<String, String>,
    },
    #[serde(rename = "chat_completions")]
    Completions {
        #[serde(default)]
        seal: Option<String>,
        #[serde(default)]
        identity: Option<String>,
        #[serde(default, rename = "api_key")]
        key: Option<String>,
        #[serde(default, rename = "base_url")]
        endpoint: Option<String>,
        #[serde(default)]
        thinking: Option<String>,
        #[serde(default)]
        replay: santi_provider::chat::completions::Replay,
        #[serde(default, rename = "reasoning_effort")]
        effort: Option<String>,
        #[serde(default, rename = "max_tokens")]
        tokens: Option<u32>,
        #[serde(default, alias = "input_budget_bytes")]
        bytes: Option<usize>,
        #[serde(default, rename = "maintenance_bytes")]
        maintenance: Option<usize>,
        #[serde(default)]
        headers: Option<Vec<String>>,
        #[serde(default, rename = "request_headers")]
        outgoing: std::collections::BTreeMap<String, String>,
    },
}

impl Profile {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Responses { .. } => "openai_responses",
            Self::Gemini { .. } => "gemini_generate_content",
            Self::Completions { .. } => "chat_completions",
        }
    }

    pub fn resolve(&self, provider: &str, model: &str) -> Result<Resolved, String> {
        match self {
            Profile::Responses {
                seal: _,
                key,
                endpoint,
                effort,
                summary,
                tokens,
                bytes,
                maintenance,
                attempts,
                headers,
                outgoing,
            } => Ok(Resolved::Responses(Responses {
                key: required(key, provider, "api_key")?,
                model: model.to_owned(),
                endpoint: optional(endpoint, provider, "base_url")?
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                effort: optional(effort, provider, "reasoning_effort")?,
                summary: optional(summary, provider, "reasoning_summary")?,
                tokens: *tokens,
                bytes: positive(*bytes, provider, "bytes")?,
                maintenance: capacity(*bytes, *maintenance, provider)?,
                attempts: *attempts,
                headers: super::headers::allowed(headers.as_deref(), provider)?,
                outgoing: super::headers::requests(outgoing, provider)?,
            })),
            Profile::Gemini {
                seal: _,
                identity: _,
                key,
                endpoint,
                effort,
                tokens,
                bytes,
                maintenance,
                headers,
                outgoing,
            } => Ok(Resolved::Gemini(Gemini {
                provider: provider.to_string(),
                key: required(key, provider, "api_key")?,
                model: model.to_owned(),
                endpoint: required(endpoint, provider, "base_url")?,
                effort: optional(effort, provider, "reasoning_effort")?,
                tokens: *tokens,
                bytes: positive(*bytes, provider, "bytes")?,
                maintenance: capacity(*bytes, *maintenance, provider)?,
                headers: super::headers::allowed(headers.as_deref(), provider)?,
                outgoing: super::headers::requests(outgoing, provider)?,
            })),
            Profile::Completions {
                seal: _,
                identity: _,
                key,
                endpoint,
                thinking,
                replay,
                effort,
                tokens,
                bytes,
                maintenance,
                headers,
                outgoing,
            } => Ok(Resolved::Completions(Completions {
                provider: provider.to_string(),
                key: required(key, provider, "api_key")?,
                model: model.to_owned(),
                endpoint: required(endpoint, provider, "base_url")?,
                thinking: optional(thinking, provider, "thinking")?,
                replay: *replay,
                effort: optional(effort, provider, "reasoning_effort")?,
                tokens: *tokens,
                bytes: positive(*bytes, provider, "bytes")?,
                maintenance: capacity(*bytes, *maintenance, provider)?,
                headers: super::headers::allowed(headers.as_deref(), provider)?,
                outgoing: super::headers::requests(outgoing, provider)?,
            })),
        }
    }
}

pub fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(|value| trimmed(&value))
}

pub fn home() -> PathBuf {
    if let Some(home) = env("SANTI_HOME") {
        return expanded(&home);
    }
    let base = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join(".santi")
}

#[derive(Debug, Clone)]
pub struct Layout {
    pub database: PathBuf,
    pub runtime: PathBuf,
    pub execution: PathBuf,
}

fn expanded(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(path)
}

fn trimmed(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}
