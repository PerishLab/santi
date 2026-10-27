use super::*;
use async_trait::async_trait;
use santi_provider::{Metadata, Provider, Request, Streaming};
use std::sync::Arc;
use tower::ServiceExt;

struct Silent;
#[async_trait]
impl Provider for Silent {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("test"),
            model: "test".into(),
            budget: None,
        }
    }
    async fn stream(&self, _: Request) -> Result<Streaming, String> {
        Err("no provider request expected".into())
    }
}

#[tokio::test]
async fn surface() {
    let root = tempfile::tempdir().unwrap();
    santi_core::Store::bootstrap(
        root.path().join("db"),
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    let service = Service::open(
        santi_core::service::Config {
            database: root.path().join("db").display().to_string(),
            runtime: root.path().join("runtime").display().to_string(),
            execution: root.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(Silent),
    )
    .await
    .unwrap();
    let strand = service.weave().await.unwrap().strand.id;
    let app = super::super::router(service);
    let endpoint = format!("/api/v1/strands/{strand}/materials");
    let response = action(&app, &format!("{endpoint}/load"), "load").await;
    assert_eq!(response["change"]["action"], "load");
    assert_eq!(response["state"]["bindings"].as_array().unwrap().len(), 1);
    let response = action(&app, &format!("{endpoint}/drop"), "drop").await;
    assert_eq!(response["change"]["previous"].as_str().unwrap().len(), 64);
    assert!(response["state"]["bindings"].as_array().unwrap().is_empty());
    let response = action(&app, &format!("{endpoint}/load"), "load").await;
    assert!(response["state"]["bindings"].as_array().unwrap().is_empty());
    for (url, status) in [
        ("/api/v1/materials?limit=1", 200),
        ("/api/v1/souls/soul_default/materials?available=true", 200),
        ("/api/v1/souls/missing/materials?available=true", 404),
        ("/api/v1/materials?reference=builtin%3Asanti", 200),
        (
            "/api/v1/materials?reference=builtin%3Asanti&blob=SKILL.md",
            200,
        ),
        ("/api/v1/materials?limit=0", 400),
    ] {
        let request = axum::http::Request::builder()
            .uri(url)
            .body(axum::body::Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status().as_u16(), status, "{url}");
        let body = axum::body::to_bytes(response.into_body(), 100000)
            .await
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        if url.ends_with("blob=SKILL.md") {
            assert!(value["text"].as_str().unwrap().starts_with("---"));
        }
    }
}

async fn action(app: &axum::Router, url: &str, operation: &str) -> serde_json::Value {
    let body = serde_json::json!({"reference":"builtin:santi","operation":operation});
    let request = axum::http::Request::builder()
        .method("POST")
        .uri(url)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);
    let body = axum::body::to_bytes(response.into_body(), 100000)
        .await
        .unwrap();
    serde_json::from_slice(&body).unwrap()
}
