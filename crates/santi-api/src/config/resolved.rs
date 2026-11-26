use super::*;

impl Resolved {
    pub fn rebase(&self, effective: &santi_core::inference::Effective) -> Result<Self, String> {
        if effective.model.trim().is_empty() {
            return Err("inference model must not be empty".to_string());
        }
        let mut next = self.clone();
        match &mut next {
            Self::Responses(config) => {
                config.model = effective.model.clone();
                config.effort = effective.effort.clone();
            }
            Self::Gemini(config) => {
                config.model = effective.model.clone();
                config.effort = effective.effort.clone();
            }
            Self::Completions(config) => {
                config.model = effective.model.clone();
                config.effort = effective.effort.clone();
            }
        }
        Ok(next)
    }

    pub fn effort(&self) -> Option<&str> {
        match self {
            Self::Responses(config) => config.effort.as_deref(),
            Self::Gemini(config) => config.effort.as_deref(),
            Self::Completions(config) => config.effort.as_deref(),
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Responses(_) => "openai_responses",
            Self::Gemini(_) => "gemini_generate_content",
            Self::Completions(_) => "chat_completions",
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::Responses(config) => &config.model,
            Self::Gemini(config) => &config.model,
            Self::Completions(config) => &config.model,
        }
    }

    pub fn maintenance(&self) -> usize {
        match self {
            Self::Responses(config) => config.maintenance,
            Self::Gemini(config) => config.maintenance,
            Self::Completions(config) => config.maintenance,
        }
    }

    pub fn bytes(&self) -> usize {
        match self {
            Self::Responses(config) => config.bytes,
            Self::Gemini(config) => config.bytes,
            Self::Completions(config) => config.bytes,
        }
    }
}
