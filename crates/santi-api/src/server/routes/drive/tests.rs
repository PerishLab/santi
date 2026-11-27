use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, StatusCode};
use santi_core::message;
use santi_estate::{Compact, Message};
use serde_json::Value;
use tower::ServiceExt;

use super::super::{router, turn::support::open};

#[tokio::test]
async fn ownership() {
    let temp = tempfile::tempdir().unwrap();
    let (store, service) = open(&temp).await;
    let parent = service.weave().await.unwrap().strand.id;
    let created = santi_core::now();
    for tag in ["message_one", "message_two"] {
        store
            .place(Message {
                tag,
                strand: &parent,
                actor: message::Role::System,
                sender: "santi",
                kind: message::Kind::Text,
                content: &message::Content::text(tag),
                state: message::State::Fixed,
                request: false,
                created: &created,
            })
            .await
            .unwrap();
    }
    let original = store
        .condense(Compact {
            tag: "compact_parent",
            strand: &parent,
            first: "message_one",
            last: "message_two",
            summary: "two messages",
            metadata: None,
            expected: None,
            created: &created,
        })
        .await
        .unwrap()
        .compact;
    let child = service.fork(&parent).await.unwrap().strand.id;
    let compacts = store.compacts(&child).await.unwrap();
    let copy = &compacts[0].id;
    assert_ne!(&original, copy);
    let app = router(service);
    let endpoint = format!("/api/v1/compacts/{copy}?page_size=1");
    let page = request(&app, Method::GET, &endpoint, StatusCode::OK).await;
    assert_eq!(page["strand"], child);
    assert_eq!(page["entries"].as_array().unwrap().len(), 1);
    assert_eq!(page["entries"][0]["text"], "message_one");
    request(
        &app,
        Method::DELETE,
        &format!("/api/v1/strands/{child}"),
        StatusCode::OK,
    )
    .await;
    drop(app);
    drop(store);
    let (_, service) = open(&temp).await;
    let app = router(service);
    let stale = request(&app, Method::GET, &endpoint, StatusCode::GONE).await;
    assert_eq!(stale["code"], "compact.retired");
    assert_eq!(stale["context"]["strand"], child);
    assert_eq!(stale["context"]["compact"], *copy);
    let live = request(
        &app,
        Method::GET,
        &format!("/api/v1/compacts/{original}"),
        StatusCode::OK,
    )
    .await;
    assert_eq!(live["strand"], parent);
    let missing = request(
        &app,
        Method::GET,
        "/api/v1/compacts/typo",
        StatusCode::NOT_FOUND,
    )
    .await;
    assert_eq!(missing["code"], "resource.not_found");
}

async fn request(app: &Router, method: Method, path: &str, status: StatusCode) -> Value {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let actual = response.status();
    let body = to_bytes(response.into_body(), 65536).await.unwrap();
    let value = serde_json::from_slice(&body).unwrap();
    assert_eq!(actual, status, "{value}");
    value
}
