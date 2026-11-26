use super::*;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use santi_core::inference::Declaration;
use santi_core::service::{self, jobs};
use std::sync::Arc;
use tower::ServiceExt;

#[path = "../../../../tests/support/catalog.rs"]
mod catalog;

struct Idle;
impl jobs::Supervisor for Idle {
    fn detach(&self, _: &jobs::Launch) -> Result<(), String> {
        Err("no jobs".into())
    }
    fn observe(&self, _: &jobs::Launch) -> Result<jobs::Observation, String> {
        Err("no jobs".into())
    }
    fn stop(&self, _: &jobs::Launch) -> Result<(), String> {
        Err("no jobs".into())
    }
    fn acknowledge(&self, _: &jobs::Launch) -> Result<(), String> {
        Err("no jobs".into())
    }
}

async fn serve(declaration: Declaration, endpoint: &str) -> (tempfile::TempDir, axum::Router) {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("db");
    let store = santi_core::Store::bootstrap(
        &database,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    catalog::qualify(
        &store,
        endpoint,
        &[
            ("routine", "service-model"),
            ("review", "seat-model"),
            ("parent", "parent-model"),
            ("alternate", "alternate-model"),
        ],
    )
    .await;
    let custody = toml::from_str(&format!(
        "[operator]\nkind='openai_responses'\napi_key='fixture'\nbytes=500000\nbase_url='{endpoint}'"
    ))
    .unwrap();
    let service = Service::provided(
        service::Config {
            database: database.display().to_string(),
            runtime: root.path().join("runtime").display().to_string(),
            execution: root.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(crate::provider::Catalog {
            custody: Box::leak(Box::new(custody)),
        }),
        declaration,
        Arc::new(Idle),
    )
    .await
    .unwrap();
    (root, super::super::router(service))
}

async fn fixture() -> (tempfile::TempDir, axum::Router) {
    serve(
        Declaration {
            scenario: Some("gateway/family/routine".into()),
            effort: None,
        },
        "https://api.openai.com/v1",
    )
    .await
}

async fn raw(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: (&str, Option<&str>),
) -> (StatusCode, serde_json::Value) {
    let (body, content_type) = body;
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(content_type) = content_type {
        request = request.header("content-type", content_type);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 100_000)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

async fn call(
    app: &axum::Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder().method(method).uri(uri);
    let body = match body {
        Some(body) => {
            request = request.header("content-type", "application/json");
            Body::from(body.to_string())
        }
        None => Body::empty(),
    };
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 100_000)
        .await
        .unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

#[path = "create.rs"]
mod create;
#[path = "media.rs"]
mod media;
#[path = "read.rs"]
mod read;
#[path = "write.rs"]
mod write;
