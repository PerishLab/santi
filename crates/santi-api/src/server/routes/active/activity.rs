use axum::{
    Json,
    extract::{Path, State},
    response::{Sse, sse::Event, sse::KeepAlive},
};
use futures_core::Stream;
use santi_core::{service::Service, turn::activity::Snapshot};
use std::{convert::Infallible, time::Duration};
use tokio::sync::watch;

use crate::server::ApiError;

#[utoipa::path(
    get, path = "/api/v1/strands/{strand}/activity", params(("strand" = String, Path)),
    summary = "Current process-local execution activity. Null current does not establish idleness or completion; inspect the exact durable receipt/turn separately. Round zero is pre-round preparation. Responding means provider activity was observed, not proof of reasoning or input consumption.",
    responses((status = 200, body = Snapshot), (status = 404, body = santi_core::Fault), (status = 500, body = santi_core::Fault))
)]
pub(in crate::server) async fn snapshot(
    State(service): State<Service>,
    Path(strand): Path<String>,
) -> Result<Json<Snapshot>, ApiError> {
    let receiver = observe(&service, &strand).await?;
    let snapshot = receiver.borrow().clone();
    Ok(Json(snapshot))
}

#[utoipa::path(
    get, path = "/api/v1/strands/{strand}/activity/events", params(("strand" = String, Path)),
    summary = "Latest-value activity subscription. Starts with the current snapshot and may coalesce intermediate changes. Epoch/revision order observations only within one service instance; reconnect starts fresh, never durable replay or Last-Event-ID recovery. Copy values before waiting; bound observation time at the client.",
    responses((status = 200, body = Snapshot, content_type = "text/event-stream"), (status = 404, body = santi_core::Fault), (status = 500, body = santi_core::Fault))
)]
pub(in crate::server) async fn events(
    State(service): State<Service>,
    Path(strand): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let mut receiver = observe(&service, &strand).await?;
    let stream = async_stream::stream! {
        let snapshot = receiver.borrow_and_update().clone();
        yield Ok(event(snapshot));
        while !service.closing() {
            let changed = tokio::select! {
                changed = receiver.changed() => changed,
                _ = tokio::time::sleep(Duration::from_millis(100)) => continue,
            };
            if changed.is_err() { break; }
            let snapshot = receiver.borrow_and_update().clone();
            yield Ok(event(snapshot));
        }
    };
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn observe(service: &Service, strand: &str) -> Result<watch::Receiver<Snapshot>, ApiError> {
    service.observe(strand).await.map_err(|error| {
        if error == "strand not found" {
            ApiError::not_found(error)
        } else {
            ApiError::from_service(error)
        }
    })
}

fn event(snapshot: Snapshot) -> Event {
    Event::default()
        .event("activity")
        .id(format!("{}:{}", snapshot.epoch, snapshot.revision))
        .json_data(snapshot)
        .expect("activity snapshot serializes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::{body::Body, http::Request};
    use futures_util::StreamExt;
    use santi_provider::{Metadata, Provider, Streaming};
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
        async fn stream(&self, _: santi_provider::Request) -> Result<Streaming, String> {
            Err("unexpected request".into())
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
        let app = super::super::super::router(service.clone());
        let endpoint = format!("/api/v1/strands/{strand}/activity");
        let request = Request::builder()
            .uri(&endpoint)
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let bytes = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let initial: Snapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(initial.strand, strand);
        assert!(initial.current.is_none());
        let request = Request::builder()
            .uri(format!("{endpoint}/events"))
            .header("Last-Event-ID", "foreign:999")
            .body(Body::empty())
            .unwrap();
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), 200);
        let mut stream = response.into_body().into_data_stream();
        let bytes = tokio::time::timeout(Duration::from_secs(2), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("event: activity"));
        let body = text
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap();
        let current: Snapshot = serde_json::from_str(body).unwrap();
        assert_eq!(current, initial);
        service.close();
        assert!(
            tokio::time::timeout(Duration::from_secs(2), stream.next())
                .await
                .unwrap()
                .is_none()
        );
        for endpoint in [
            "/api/v1/strands/missing/activity",
            "/api/v1/strands/missing/activity/events",
        ] {
            let request = Request::builder()
                .uri(endpoint)
                .body(Body::empty())
                .unwrap();
            assert_eq!(app.clone().oneshot(request).await.unwrap().status(), 404);
        }
        let text = crate::server::export_openapi_json().unwrap();
        let document: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(
            document["paths"]
                .get("/api/v1/strands/{strand}/activity/events")
                .is_some()
        );
        assert!(
            document["components"]["schemas"]
                .get("turn.activity.Snapshot")
                .is_some()
        );
    }
}
