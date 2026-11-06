use super::*;
use async_trait::async_trait;
use santi_estate::{Acceptance, InboxDraft, ReplayDraft, Store, StrandDraft, WebhookDraft};
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
        Err("no provider call expected".into())
    }
}

async fn fixture() -> (tempfile::TempDir, Service) {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("db");
    let store = Store::bootstrap(
        &database,
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    )
    .await
    .unwrap();
    let now = santi_core::now();
    store.seed("soul_default", &now).await.unwrap();
    store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_default",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: &now,
        })
        .await
        .unwrap();
    store
        .subscribe(WebhookDraft {
            name: "feishu",
            adaptor: "feishu",
            soul: "soul_default",
            strategy: "single",
            credential: "TEST_SECRET",
            created: &now,
        })
        .await
        .unwrap();
    store
        .windowed(
            InboxDraft {
                tag: "inbox_test",
                strand: "strand_test",
                kind: santi_core::message::Kind::Text,
                content: &santi_core::message::Content::text("input"),
                source: None,
                created: &now,
            },
            Acceptance {
                replay: ReplayDraft::Webhook {
                    subscription: "feishu",
                    delivery: "event",
                    digest: "digest",
                },
                gate: 10,
                binding: Some(&Binding {
                    adaptor: "feishu".into(),
                    context: json!({}),
                    delay: 3000,
                }),
            },
        )
        .await
        .unwrap();
    drop(store);
    let service = Service::open(
        santi_core::service::Config {
            database: database.display().to_string(),
            runtime: directory.path().join("runtime").display().to_string(),
            execution: directory.path().join("execution").display().to_string(),
            bind: None,
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(Silent),
    )
    .await
    .unwrap();
    (directory, service)
}

async fn request(app: &axum::Router, method: &str, uri: &str, body: Value) -> (u16, Value) {
    let request = axum::http::Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(body.to_string()))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let body = axum::body::to_bytes(response.into_body(), 100_000)
        .await
        .unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn scope() {
    let (_directory, service) = fixture().await;
    let app = crate::server::routes::router(service.clone());
    let endpoint = "/api/v1/receipts/inbox_test/reply";
    let mut intent = json!({"strand":"strand_foreign", "operation":"final", "text":"final text"});
    assert_eq!(request(&app, "POST", endpoint, intent.clone()).await.0, 400);
    intent["strand"] = json!("strand_test");
    let (status, first) = request(&app, "POST", endpoint, intent.clone()).await;
    assert_eq!(status, 200);
    assert_eq!(first["window"]["owner"], "soul");
    let (status, replay) = request(&app, "POST", endpoint, intent.clone()).await;
    assert_eq!(status, 200);
    assert_eq!(first, replay);
    intent["text"] = json!("changed");
    assert_eq!(request(&app, "POST", endpoint, intent.clone()).await.0, 400);
    intent["operation"] = json!("second");
    assert_eq!(request(&app, "POST", endpoint, intent.clone()).await.0, 400);
    let attempt = service.outgoing("inbox_test").await.unwrap().unwrap();
    assert_eq!(attempt.owner, Owner::Soul);
    assert!(attempt.target.is_none());
    service
        .delivered(
            "inbox_test",
            santi_estate::Outcome {
                operation: "final",
                result: &json!({"state":"accepted"}),
                message: Some("om_final"),
            },
        )
        .await
        .unwrap();
    let (status, accepted) = request(&app, "GET", endpoint, Value::Null).await;
    assert_eq!(status, 200);
    assert_eq!(accepted["window"]["message"], "om_final");
    assert_eq!(accepted["window"]["attempts"][0]["state"], "accepted");
    assert_eq!(
        request(&app, "GET", "/api/v1/receipts/missing/reply", Value::Null)
            .await
            .0,
        404
    );
}
