use super::*;

async fn strands(app: &axum::Router) -> usize {
    let (status, strands) = call(app, "GET", "/api/v1/strands", None).await;
    assert_eq!(status, StatusCode::OK, "{strands}");
    strands.as_array().expect("strands").len()
}

#[tokio::test]
async fn seat() {
    let (_root, app) = fixture().await;
    let (status, created) = call(&app, "POST", "/api/v1/strands", None).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let soul = created["strand"]["soul"].as_str().unwrap();

    let (status, omitted) =
        call(&app, "POST", &format!("/api/v1/souls/{soul}/strands"), None).await;
    assert_eq!(status, StatusCode::OK, "{omitted}");
    let omitted = omitted["strand"]["id"].as_str().unwrap();

    let (status, null) = call(
        &app,
        "POST",
        &format!("/api/v1/souls/{soul}/strands"),
        Some(serde_json::Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{null}");
    let null = null["strand"]["id"].as_str().unwrap();

    let (status, declared) = call(
        &app,
        "POST",
        &format!("/api/v1/souls/{soul}/strands"),
        Some(serde_json::json!({"inference": {"model": "seat-model"}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{declared}");
    let declared = declared["strand"]["id"].as_str().unwrap();

    for strand in [omitted, null] {
        let (status, reading) = call(
            &app,
            "GET",
            &format!("/api/v1/strands/{strand}/inference"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{reading}");
        assert_eq!(reading["declaration"], serde_json::json!({}));
        assert_eq!(reading["effective"]["model"], "service-model");
        assert_eq!(reading["effective"]["source"]["model"], "service");
    }

    let (status, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{declared}/inference"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reading}");
    assert_eq!(reading["declaration"]["model"], "seat-model");
    assert_eq!(reading["effective"]["model"], "seat-model");
    assert_eq!(reading["effective"]["source"]["model"], "strand");
}

#[tokio::test]
async fn weave() {
    let (_root, app) = fixture().await;
    let before = strands(&app).await;

    let (status, omitted) = call(&app, "POST", "/api/v1/strands", None).await;
    assert_eq!(status, StatusCode::OK, "{omitted}");
    let (status, null) = call(
        &app,
        "POST",
        "/api/v1/strands",
        Some(serde_json::Value::Null),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{null}");
    assert_eq!(strands(&app).await, before + 2);

    for created in [omitted, null] {
        let strand = created["strand"]["id"].as_str().unwrap();
        let (status, reading) = call(
            &app,
            "GET",
            &format!("/api/v1/strands/{strand}/inference"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{reading}");
        assert_eq!(reading["declaration"], serde_json::json!({}));
    }
}

#[tokio::test]
async fn fork() {
    let (_root, app) = fixture().await;
    let (status, parent) = call(
        &app,
        "POST",
        "/api/v1/strands",
        Some(serde_json::json!({"inference": {"model": "parent-model"}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{parent}");
    let parent = parent["strand"]["id"].as_str().unwrap();

    for body in [None, Some(serde_json::Value::Null)] {
        let (status, child) = call(
            &app,
            "POST",
            &format!("/api/v1/strands/{parent}/fork"),
            body,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{child}");
        let child = child["strand"]["id"].as_str().unwrap();
        let (status, reading) = call(
            &app,
            "GET",
            &format!("/api/v1/strands/{child}/inference"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{reading}");
        assert_eq!(reading["declaration"]["model"], "parent-model");
        assert_eq!(reading["effective"]["model"], "parent-model");
        assert_eq!(reading["effective"]["source"]["model"], "strand");
    }

    let (status, child) = call(
        &app,
        "POST",
        &format!("/api/v1/strands/{parent}/fork"),
        Some(serde_json::json!({"inference": {}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{child}");
    let child = child["strand"]["id"].as_str().unwrap();
    let (status, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{child}/inference"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reading}");
    assert_eq!(reading["declaration"], serde_json::json!({}));
    assert_eq!(reading["effective"]["model"], "service-model");
    assert_eq!(reading["effective"]["source"]["model"], "service");
}

#[tokio::test]
async fn invalid() {
    let (_root, app) = fixture().await;
    let (status, created) = call(&app, "POST", "/api/v1/strands", None).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let soul = created["strand"]["soul"].as_str().unwrap();
    let before = strands(&app).await;

    for body in [
        serde_json::json!({"inference": {"unknown": true}}),
        serde_json::json!({"inference": {"model": ""}}),
    ] {
        let (status, error) = call(
            &app,
            "POST",
            &format!("/api/v1/souls/{soul}/strands"),
            Some(body),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    }
    let (status, error) = raw(
        &app,
        "POST",
        &format!("/api/v1/souls/{soul}/strands"),
        ("{", Some("application/json")),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    assert_eq!(strands(&app).await, before);
}
