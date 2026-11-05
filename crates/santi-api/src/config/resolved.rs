use super::*;

impl Resolved {
    pub fn rebase(&self, effective: &santi_core::inference::Effective) -> Result<Self, String> {
        if effective.model.trim().is_empty() {
            return Err("inference model must not be empty".to_string());
        }
        let mut next = self.clone();
        match &mut next {
            Self::OpenAiResponses(config) => {
                config.model = effective.model.clone();
                config.reasoning_effort = effective.reasoning_effort.clone();
            }
            Self::ChatCompletions(config) => {
                config.model = effective.model.clone();
                config.reasoning_effort = effective.reasoning_effort.clone();
            }
        }
        Ok(next)
    }

    pub fn effort(&self) -> Option<&str> {
        match self {
            Self::OpenAiResponses(config) => config.reasoning_effort.as_deref(),
            Self::ChatCompletions(config) => config.reasoning_effort.as_deref(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::OpenAiResponses(_) => "openai_responses",
            Self::ChatCompletions(_) => "chat_completions",
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::OpenAiResponses(config) => &config.model,
            Self::ChatCompletions(config) => &config.model,
        }
    }

    pub fn maintenance(&self) -> usize {
        match self {
            Self::OpenAiResponses(config) => config.maintenance,
            Self::ChatCompletions(config) => config.maintenance,
        }
    }

    pub fn bytes(&self) -> usize {
        match self {
            Self::OpenAiResponses(config) => config.bytes,
            Self::ChatCompletions(config) => config.bytes,
        }
    }
}
