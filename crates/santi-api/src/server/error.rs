use axum::{Json, http::StatusCode, response::IntoResponse};
use santi_core::{Category, Fault, Ruled, Signal, budget, catalog, engine};

use crate::webhook::WebhookError;

pub struct ApiError {
    status: StatusCode,
    error: Fault,
}

enum Kind {
    Down,
    Taken,
    Fault,
    Unauthorized,
    Unavailable,
    Missing,
    Conflict,
    Invalid,
    Internal,
}

enum Rule {
    Exact(&'static str),
    Starts(&'static str),
    Contains(&'static str),
    Ends(&'static str),
    Around {
        start: &'static str,
        fragment: &'static str,
    },
}

impl Rule {
    fn accepts(&self, text: &str) -> bool {
        match self {
            Self::Exact(value) => text == *value,
            Self::Starts(value) => text.starts_with(value),
            Self::Contains(value) => text.contains(value),
            Self::Ends(value) => text.ends_with(value),
            Self::Around { start, fragment } => text.starts_with(start) && text.contains(fragment),
        }
    }
}

const AUTH: &[Rule] = &[
    Rule::Exact("invalid job capability"),
    Rule::Exact("job capability expired"),
];

const UNAVAILABLE: &[Rule] = &[
    Rule::Starts("job supervisor is unavailable"),
    Rule::Starts("systemd did not accept job"),
    Rule::Starts("launchd did not accept job"),
    Rule::Contains("sidecar did not claim"),
    Rule::Contains("sidecar failed before claimed"),
];

const MISSING: &[Rule] = &[
    Rule::Starts("missing resource:"),
    Rule::Exact("strand not found"),
    Rule::Contains("alarm not found or ended"),
    Rule::Exact("soul not found"),
    Rule::Ends("not found"),
];

const CONFLICT: &[Rule] = &[
    Rule::Contains("alarm generation conflicts"),
    Rule::Contains("compact plan conflicts"),
    Rule::Starts("downstream request conflicts"),
    Rule::Starts("downstream id conflicts"),
    Rule::Starts("webhook delivery conflicts"),
    Rule::Around {
        start: "webhook ",
        fragment: " conflicts ",
    },
    Rule::Starts("job capability conflicts"),
    Rule::Starts("job execution spec conflicts"),
    Rule::Contains("sidecar stamp conflicts"),
    Rule::Contains("overlaps an existing registration"),
    Rule::Ends("is already registered"),
];

const INVALID: &[Rule] = &[
    Rule::Starts("alarm "),
    Rule::Starts("[compact."),
    Rule::Starts("compact summary is"),
    Rule::Starts("context refused:"),
    Rule::Starts("unknown soul"),
    Rule::Contains("must not be empty"),
    Rule::Around {
        start: "downstream ",
        fragment: " must ",
    },
    Rule::Contains("must contain text"),
    Rule::Starts("strategy must be"),
    Rule::Starts("fork"),
    Rule::Contains(" is past parent end "),
    Rule::Contains("object key"),
    Rule::Contains("object uri"),
    Rule::Contains("path segment"),
    Rule::Contains("path separators"),
    Rule::Starts("only an unknown effect"),
    Rule::Starts("effect resolution evidence"),
    Rule::Around {
        start: "job ",
        fragment: " must ",
    },
    Rule::Starts("environment "),
    Rule::Starts("only a terminal job"),
];

fn accepts(text: &str, rules: &[Rule]) -> bool {
    rules.iter().any(|rule| rule.accepts(text))
}

fn kind(text: &str) -> Kind {
    match text {
        "store unavailable" => return Kind::Down,
        "key taken" => return Kind::Taken,
        "store fault" => return Kind::Fault,
        _ => {}
    }
    if accepts(text, AUTH) {
        return Kind::Unauthorized;
    }
    if accepts(text, UNAVAILABLE) {
        return Kind::Unavailable;
    }
    if accepts(text, MISSING) {
        return Kind::Missing;
    }
    if accepts(text, CONFLICT) {
        return Kind::Conflict;
    }
    if accepts(text, INVALID) {
        return Kind::Invalid;
    }
    Kind::Internal
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn code(&self) -> &str {
        &self.error.code
    }

    pub fn message(&self) -> &str {
        &self.error.message
    }

    pub fn internal(message: String) -> Self {
        eprintln!("santi-api: internal error: {message}");
        Self::fault("internal error")
    }

    pub fn fault(message: impl Into<String>) -> Self {
        Self::from_santi(engine().transient(Signal {
            descriptor: catalog::INTERNAL,
            source: santi_core::Source::new("santi-api", "http_boundary"),
            scope: None,
            message: message.into(),
            context: serde_json::Value::Null,
        }))
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::from_santi(engine().transient(Signal {
            descriptor: catalog::NOT_FOUND,
            source: santi_core::Source::new("santi-api", "http_boundary"),
            scope: None,
            message: message.into(),
            context: serde_json::Value::Null,
        }))
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::from_santi(engine().transient(Signal {
            descriptor: catalog::INVALID_ARGUMENT,
            source: santi_core::Source::new("santi-api", "http_boundary"),
            scope: None,
            message: message.into(),
            context: serde_json::Value::Null,
        }))
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::from_santi(engine().transient(Signal {
            descriptor: catalog::UNAUTHORIZED,
            source: santi_core::Source::new("santi-api", "http_boundary"),
            scope: None,
            message: message.into(),
            context: serde_json::Value::Null,
        }))
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        let mut error = Self::unauthorized(message);
        error.status = StatusCode::FORBIDDEN;
        error
    }

    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::from_santi(engine().transient(Signal {
            descriptor: catalog::UNAVAILABLE,
            source: santi_core::Source::new("santi-api", "http_boundary"),
            scope: None,
            message: message.into(),
            context: serde_json::Value::Null,
        }))
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        let mut error = Self::bad_request(message);
        error.status = StatusCode::CONFLICT;
        error
    }

    pub fn from_santi(error: Fault) -> Self {
        let status = if error.code == budget::Error::Context.descriptor().code {
            StatusCode::LOCKED
        } else {
            match error.category {
                Category::Internal => StatusCode::INTERNAL_SERVER_ERROR,
                Category::Invalid => StatusCode::BAD_REQUEST,
                Category::Missing => StatusCode::NOT_FOUND,
                Category::Exhausted => StatusCode::TOO_MANY_REQUESTS,
                Category::Unauthorized => StatusCode::UNAUTHORIZED,
                Category::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
            }
        };
        Self { status, error }
    }

    pub fn from_webhook(error: WebhookError) -> Self {
        match error {
            WebhookError::Unauthorized(message) => Self::unauthorized(message),
            WebhookError::BadRequest(message) => Self::bad_request(message),
            WebhookError::Unavailable(message) => Self::unavailable(message),
        }
    }

    pub fn from_service(message: String) -> Self {
        let detail = message.strip_prefix("adapt: ").unwrap_or(&message);
        match kind(detail) {
            Kind::Down => Self::unavailable(detail),
            Kind::Taken => Self::conflict(detail),
            Kind::Fault => Self::fault(detail),
            Kind::Unauthorized => Self::unauthorized(detail),
            Kind::Unavailable => Self::unavailable(detail),
            Kind::Missing => Self::not_found(detail),
            Kind::Conflict => Self::conflict(detail),
            Kind::Invalid => Self::bad_request(detail),
            Kind::Internal => Self::internal(message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let retry_after = self
            .error
            .context
            .get("retry_after_seconds")
            .and_then(serde_json::Value::as_u64);
        let mut response = (self.status, Json(self.error)).into_response();
        if let Some(seconds) = retry_after
            && let Ok(value) = axum::http::HeaderValue::from_str(&seconds.to_string())
        {
            response.headers_mut().insert("retry-after", value);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict() {
        for prefix in ["", "adapt: "] {
            let error = ApiError::from_service(format!(
                "{prefix}job capability conflicts with its accepted request"
            ));
            assert_eq!(error.status(), StatusCode::CONFLICT);
            assert_eq!(
                error.message(),
                "job capability conflicts with its accepted request"
            );
        }
    }
}
