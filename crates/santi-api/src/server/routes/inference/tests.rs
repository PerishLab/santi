use super::*;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use santi_core::inference::{Declaration, Defaults, Directory, Effective, Origin};
use santi_core::provider::Resolver;
use santi_core::service::{self, JobLaunch, JobObservation, JobSupervisor};
use santi_provider::{Metadata, Provider, Streaming};
use std::sync::Arc;
use tower::ServiceExt;

struct Unused;
#[async_trait]
impl Provider for Unused {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("test"),
            model: "test".into(),
            budget: None,
        }
    }
    async fn stream(&self, _: santi_provider::Request) -> Result<Streaming, String> {
        Err("no provider request expected".into())
    }
}

struct Catalog;
impl Directory for Catalog {
    fn defaults(&self, provider: &str) -> Result<Defaults, String> {
        match provider {
            "test" => Ok(Defaults {
                model: "default-test".into(),
                reasoning_effort: None,
            }),
            "alternate" => Ok(Defaults {
                model: "default-alternate".into(),
                reasoning_effort: Some("low".into()),
            }),
            _ => Err(format!("unknown provider {provider}")),
        }
    }
}
impl Resolver for Catalog {
    fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
        Effective::resolve(layers, self)
    }
    fn build(&self, _: &Effective) -> Result<Arc<dyn Provider>, String> {
        Ok(Arc::new(Unused))
    }
}

struct Idle;
impl JobSupervisor for Idle {
    fn detach(&self, _: &JobLaunch) -> Result<(), String> {
        Err("no jobs".into())
    }
    fn observe(&self, _: &JobLaunch) -> Result<JobObservation, String> {
        Err("no jobs".into())
    }
    fn stop(&self, _: &JobLaunch) -> Result<(), String> {
        Err("no jobs".into())
    }
    fn acknowledge(&self, _: &JobLaunch) -> Result<(), String> {
        Err("no jobs".into())
    }
}

async fn serve(declaration: Declaration) -> (tempfile::TempDir, axum::Router) {
    let root = tempfile::tempdir().unwrap();
    let database = root.path().join("db");
    santi_core::Store::bootstrap(
        &database,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
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
        Arc::new(Catalog),
        declaration,
        Arc::new(Idle),
    )
    .await
    .unwrap();
    (root, super::super::router(service))
}

async fn fixture() -> (tempfile::TempDir, axum::Router) {
    serve(Declaration {
        provider: Some("test".into()),
        model: Some("service-model".into()),
        reasoning_effort: None,
    })
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

#[path = "media.rs"]
mod media;
#[path = "read.rs"]
mod read;
#[path = "write.rs"]
mod write;
