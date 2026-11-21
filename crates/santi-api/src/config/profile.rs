mod fields;
mod secrecy;
use super::*;
use fields::{capacity, optional, positive, required};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    OpenAiResponses(OpenAiResponses),
    ChatCompletions(ChatCompletions),
    Gemini(Gemini),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAiResponses {
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub reasoning_effort: Option<String>,
    pub summary: Option<String>,
    pub max_output_tokens: Option<u32>,
    pub bytes: usize,
    pub maintenance: usize,
    pub attempts: Option<u32>,
    pub headers: Vec<String>,
    pub outgoing: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gemini {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub reasoning_effort: Option<String>,
    pub max_tokens: Option<u32>,
    pub bytes: usize,
    pub maintenance: usize,
    pub headers: Vec<String>,
    pub outgoing: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatCompletions {
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub thinking: Option<String>,
    pub replay: santi_provider::chat::completions::Replay,
    pub reasoning_effort: Option<String>,
    pub max_tokens: Option<u32>,
    pub bytes: usize,
    pub maintenance: usize,
    pub headers: Vec<String>,
    pub outgoing: std::collections::BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Profile {
    #[serde(rename = "openai_responses")]
    OpenAiResponses {
        #[serde(default)]
        seal: Option<String>,
        #[serde(default)]
        api_key: Option<String>,
        #[serde(default)]
        base_url: Option<String>,
        #[serde(default)]
        reasoning_effort: Option<String>,
        #[serde(default, alias = "reasoning_summary")]
        summary: Option<String>,
        #[serde(default)]
        max_output_tokens: Option<u32>,
        #[serde(default, alias = "input_budget_bytes")]
        bytes: Option<usize>,
        #[serde(default)]
        maintenance_bytes: Option<usize>,
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
        #[serde(default)]
        api_key: Option<String>,
        #[serde(default)]
        base_url: Option<String>,
        #[serde(default)]
        reasoning_effort: Option<String>,
        #[serde(default)]
        max_tokens: Option<u32>,
        #[serde(default, alias = "input_budget_bytes")]
        bytes: Option<usize>,
        #[serde(default)]
        maintenance_bytes: Option<usize>,
        #[serde(default)]
        headers: Option<Vec<String>>,
        #[serde(default, rename = "request_headers")]
        outgoing: std::collections::BTreeMap<String, String>,
    },
    ChatCompletions {
        #[serde(default)]
        seal: Option<String>,
        #[serde(default)]
        identity: Option<String>,
        #[serde(default)]
        api_key: Option<String>,
        #[serde(default)]
        base_url: Option<String>,
        #[serde(default)]
        thinking: Option<String>,
        #[serde(default)]
        replay: santi_provider::chat::completions::Replay,
        #[serde(default)]
        reasoning_effort: Option<String>,
        #[serde(default)]
        max_tokens: Option<u32>,
        #[serde(default, alias = "input_budget_bytes")]
        bytes: Option<usize>,
        #[serde(default)]
        maintenance_bytes: Option<usize>,
        #[serde(default)]
        headers: Option<Vec<String>>,
        #[serde(default, rename = "request_headers")]
        outgoing: std::collections::BTreeMap<String, String>,
    },
}

impl Profile {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OpenAiResponses { .. } => "openai_responses",
            Self::Gemini { .. } => "gemini_generate_content",
            Self::ChatCompletions { .. } => "chat_completions",
        }
    }

    pub fn resolve(&self, provider: &str, model: &str) -> Result<Resolved, String> {
        match self {
            Profile::OpenAiResponses {
                seal: _,
                api_key,
                base_url,
                reasoning_effort,
                summary,
                max_output_tokens,
                bytes,
                maintenance_bytes,
                attempts,
                headers,
                outgoing,
            } => Ok(Resolved::OpenAiResponses(OpenAiResponses {
                api_key: required(api_key, provider, "api_key")?,
                model: model.to_owned(),
                base_url: optional(base_url, provider, "base_url")?
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                reasoning_effort: optional(reasoning_effort, provider, "reasoning_effort")?,
                summary: optional(summary, provider, "reasoning_summary")?,
                max_output_tokens: *max_output_tokens,
                bytes: positive(*bytes, provider, "bytes")?,
                maintenance: capacity(*bytes, *maintenance_bytes, provider)?,
                attempts: *attempts,
                headers: super::headers::allowed(headers.as_deref(), provider)?,
                outgoing: super::headers::requests(outgoing, provider)?,
            })),
            Profile::Gemini {
                seal: _,
                identity: _,
                api_key,
                base_url,
                reasoning_effort,
                max_tokens,
                bytes,
                maintenance_bytes,
                headers,
                outgoing,
            } => Ok(Resolved::Gemini(Gemini {
                provider: provider.to_string(),
                api_key: required(api_key, provider, "api_key")?,
                model: model.to_owned(),
                base_url: required(base_url, provider, "base_url")?,
                reasoning_effort: optional(reasoning_effort, provider, "reasoning_effort")?,
                max_tokens: *max_tokens,
                bytes: positive(*bytes, provider, "bytes")?,
                maintenance: capacity(*bytes, *maintenance_bytes, provider)?,
                headers: super::headers::allowed(headers.as_deref(), provider)?,
                outgoing: super::headers::requests(outgoing, provider)?,
            })),
            Profile::ChatCompletions {
                seal: _,
                identity: _,
                api_key,
                base_url,
                thinking,
                replay,
                reasoning_effort,
                max_tokens,
                bytes,
                maintenance_bytes,
                headers,
                outgoing,
            } => Ok(Resolved::ChatCompletions(ChatCompletions {
                provider: provider.to_string(),
                api_key: required(api_key, provider, "api_key")?,
                model: model.to_owned(),
                base_url: required(base_url, provider, "base_url")?,
                thinking: optional(thinking, provider, "thinking")?,
                replay: *replay,
                reasoning_effort: optional(reasoning_effort, provider, "reasoning_effort")?,
                max_tokens: *max_tokens,
                bytes: positive(*bytes, provider, "bytes")?,
                maintenance: capacity(*bytes, *maintenance_bytes, provider)?,
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
