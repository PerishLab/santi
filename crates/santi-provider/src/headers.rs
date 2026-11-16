use radicula_template::Template;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub struct Headers {
    templates: Vec<(HeaderName, Template)>,
    agent: String,
}

impl Headers {
    pub fn new(values: &BTreeMap<String, String>, agent: &str) -> Result<Self, String> {
        let mut headers = Self {
            templates: Vec::new(),
            agent: agent.into(),
        };
        for (name, value) in values {
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| "request_headers contains an invalid header name".to_string())?;
            if Self::reserved(name.as_str()) {
                return Err(format!("request_headers may not override {name}"));
            }
            if headers.templates.iter().any(|(held, _)| held == name) {
                return Err(format!("request_headers repeats {name}"));
            }
            HeaderValue::from_str(value)
                .map_err(|_| format!("request_headers {name} contains an invalid header value"))?;
            let template = Template::new(value, &["meta.user_agent", "strand.id"])
                .map_err(|error| format!("request_headers {name}: {error}"))?;
            headers.templates.push((name, template));
        }
        Ok(headers)
    }

    pub fn render(&self, strand: Option<&str>) -> Result<HeaderMap, String> {
        let mut vars = BTreeMap::from([("meta.user_agent", self.agent.clone())]);
        if let Some(strand) = strand.filter(|value| !value.is_empty()) {
            vars.insert("strand.id", strand.into());
        }
        let mut headers = HeaderMap::new();
        for (name, template) in &self.templates {
            let rendered = template
                .render(&vars)
                .map_err(|error| format!("request_headers {name}: {error}"))?;
            let mut value = HeaderValue::from_str(&rendered)
                .map_err(|_| format!("request_headers {name} produced an invalid header value"))?;
            value.set_sensitive(true);
            headers.insert(name.clone(), value);
        }
        Ok(headers)
    }

    fn reserved(name: &str) -> bool {
        matches!(
            name,
            "authorization"
                | "proxy-authorization"
                | "host"
                | "content-type"
                | "content-encoding"
                | "content-length"
                | "transfer-encoding"
                | "connection"
                | "keep-alive"
                | "proxy-connection"
                | "te"
                | "trailer"
                | "upgrade"
        )
    }
}

impl std::fmt::Debug for Headers {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.debug_struct("Headers")
            .field("count", &self.templates.len())
            .finish_non_exhaustive()
    }
}
