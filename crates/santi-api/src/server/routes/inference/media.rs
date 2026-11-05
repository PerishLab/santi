use super::*;

#[tokio::test]
async fn bodies() {
    let (_root, app) = fixture().await;
    let (_, before) = call(&app, "GET", "/api/v1/strands", None).await;
    let count = before.as_array().unwrap().len();
    let draft = r#"{"inference":{}}"#;

    for content_type in [None, Some("text/plain")] {
        let (status, _) = raw(&app, "POST", "/api/v1/strands", (draft, content_type)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{content_type:?}");
    }
    let (status, _) = raw(
        &app,
        "POST",
        "/api/v1/strands",
        ("{", Some("application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = raw(
        &app,
        "POST",
        "/api/v1/strands",
        (r#"{"unknown":1}"#, Some("application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = raw(
        &app,
        "POST",
        "/api/v1/strands",
        (
            r#"{"inference":{"provider":"missing"}}"#,
            Some("application/json"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, after) = call(&app, "GET", "/api/v1/strands", None).await;
    assert_eq!(after.as_array().unwrap().len(), count);

    let (status, created) = raw(
        &app,
        "POST",
        "/api/v1/strands",
        ("", Some("application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let strand = created["strand"]["id"].as_str().unwrap();
    let soul = created["strand"]["soul"].as_str().unwrap();

    let (status, _) = raw(
        &app,
        "POST",
        &format!("/api/v1/souls/{soul}/strands"),
        (draft, None),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = raw(
        &app,
        "POST",
        &format!("/api/v1/strands/{strand}/fork"),
        (draft, None),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &app,
        "POST",
        &format!("/api/v1/strands/{strand}/fork"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}
