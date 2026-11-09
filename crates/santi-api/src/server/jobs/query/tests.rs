use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use santi_core::{
    Store,
    service::{Config, Service},
    turn,
};
use santi_estate::{
    CallDraft, CapabilityDraft, EffectDraft, JobDraft, Prepared, StrandDraft, TurnDraft,
};
use santi_provider::{Event, Metadata, Provider, Request as Ask, Streaming};
use tower::ServiceExt;

const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const AT: &str = "2026-11-09T00:00:00.000Z";

struct Test;

#[async_trait::async_trait]
impl Provider for Test {
    fn metadata(&self) -> Metadata {
        Metadata {
            provider: Arc::from("test"),
            model: "test".to_string(),
            budget: None,
        }
    }

    async fn stream(&self, _: Ask) -> Result<Streaming, String> {
        Ok(Box::pin(futures_util::stream::iter(vec![Ok(
            Event::Completed { response: None },
        )])))
    }
}

async fn seed(temp: &tempfile::TempDir) -> String {
    let database = temp.path().join("santi.sqlite");
    let store = Store::bootstrap(&database, SUDO).await.expect("bootstrap");
    for soul in ["soul-a", "soul-b"] {
        store.seed(soul, AT).await.expect("seed");
    }
    for (soul, tag) in [("soul-a", "a1"), ("soul-a", "a2"), ("soul-b", "b1")] {
        let strand = store
            .create_strand(StrandDraft {
                tag,
                soul,
                label: None,
                parent: None,
                fork: None,
                inference: None,
                created: AT,
            })
            .await
            .expect("strand");
        let turn = store
            .create_turn(TurnDraft {
                tag: &format!("{tag}-turn"),
                strand: &strand.id,
                trigger: turn::Trigger::System,
                from: 0,
                created: AT,
            })
            .await
            .expect("turn");
        let call = store
            .create_call(CallDraft {
                tag: &format!("{tag}-call"),
                turn: &turn.id,
                tool: "shell",
                arguments: &serde_json::json!({ "command": "true" }),
                created: AT,
            })
            .await
            .expect("call");
        let effect = store
            .prepare_effect(EffectDraft {
                tag: &format!("{tag}-effect"),
                turn: &turn.id,
                call: Some(&call.id),
                kind: "shell",
                metadata: None,
                created: AT,
            })
            .await
            .expect("effect");
        let capability = format!("{tag}-cap");
        store
            .create_capability(CapabilityDraft {
                digest: &capability,
                expires: 1_000,
                soul,
                strand: &strand.id,
                turn: &turn.id,
                call: &call.id,
                effect: &effect.id,
                created: AT,
            })
            .await
            .expect("capability");
        let prepared = store
            .prepare_job(
                &capability,
                JobDraft {
                    environment: None,
                    tag: &format!("{tag}-job"),
                    description: "test job",
                    command: "true",
                    cwd: None,
                    timeout_seconds: 30,
                    output_limit_bytes: 1024,
                    remind_every_seconds: None,
                    notify_on_terminal: false,
                    request_sha256: &format!("{tag}-request"),
                    generation: &format!("{tag}-generation"),
                    supervisor_ref: &format!("santi-{tag}.service"),
                    created: AT,
                },
                1_000,
            )
            .await
            .expect("job");
        assert!(matches!(prepared, Prepared::New(_)));
    }
    database.display().to_string()
}

async fn app(temp: &tempfile::TempDir) -> Router {
    let database = seed(temp).await;
    let service = Service::open(
        Config {
            database,
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(Test),
    )
    .await
    .expect("service");
    crate::server::routes::router(service)
}

fn request(soul: Option<&str>, body: serde_json::Value) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri("/api/v1/query")
        .header("content-type", "application/json");
    if let Some(soul) = soul {
        builder = builder.header("x-santi-soul-id", soul);
    }
    builder.body(Body::from(body.to_string())).expect("request")
}

async fn send(
    app: &Router,
    soul: Option<&str>,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let response = app
        .clone()
        .oneshot(request(soul, body))
        .await
        .expect("response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    (status, value)
}

#[tokio::test]
async fn http() {
    let temp = tempfile::tempdir().expect("temp");
    let app = app(&temp).await;

    let (status, _) = send(
        &app,
        None,
        serde_json::json!({ "all_strands": true, "count": true }),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = send(
        &app,
        Some("soul-a"),
        serde_json::json!({ "limit": 5, "after": 1 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert!(body.get("message").is_some(), "{body}");

    let (status, body) = send(
        &app,
        Some("soul-a"),
        serde_json::json!({ "q": "from Job limit 5", "all_strands": true }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = send(
        &app,
        Some("soul-a"),
        serde_json::json!({ "q": "from Job where supervisor_ref = \"x\" limit 5" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = send(
        &app,
        Some("soul-a"),
        serde_json::json!({ "all_strands": true, "count": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "count": 2 }));

    let (status, body) = send(
        &app,
        Some("soul-b"),
        serde_json::json!({ "all_strands": true, "count": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "count": 1 }));

    let (status, body) = send(
        &app,
        Some("soul-a"),
        serde_json::json!({ "all_strands": true, "limit": 20 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rows = body
        .get("rows")
        .and_then(|rows| rows.as_array())
        .expect("rows");
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.get("id").is_some()));

    let (status, body) = send(
        &app,
        Some("soul-a"),
        serde_json::json!({ "strand": "a1", "limit": 20 }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rows = body
        .get("rows")
        .and_then(|rows| rows.as_array())
        .expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get("id"), Some(&serde_json::json!("a1-job")));

    let (status, body) = send(
        &app,
        Some("soul-a"),
        serde_json::json!({ "q": "from Job count" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "count": 2 }));
}
