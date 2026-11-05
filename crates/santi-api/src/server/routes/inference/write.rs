use super::*;

#[tokio::test]
async fn patches() {
    let (_root, app) = fixture().await;
    let (status, created) = call(&app, "POST", "/api/v1/strands", None).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let strand = created["strand"]["id"].as_str().unwrap().to_string();
    let soul = created["strand"]["soul"].as_str().unwrap().to_string();

    let (status, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/souls/{soul}/inference"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reading["effective"]["provider"], "test");
    assert_eq!(reading["effective"]["model"], "service-model");
    assert_eq!(reading["effective"]["source"]["provider"], "service");
    assert_eq!(reading["effective"]["source"]["model"], "service");

    let (status, reading) = call(
        &app,
        "PATCH",
        &format!("/api/v1/souls/{soul}/inference"),
        Some(serde_json::json!({"model": "soul-model"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reading}");
    assert_eq!(reading["declaration"]["model"], "soul-model");
    assert_eq!(reading["effective"]["model"], "soul-model");
    assert_eq!(reading["effective"]["source"]["model"], "soul");

    let (status, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{strand}/inference"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reading["declaration"], serde_json::json!({}));
    assert_eq!(reading["effective"]["model"], "soul-model");
    assert_eq!(reading["effective"]["source"]["model"], "soul");

    let (status, reading) = call(
        &app,
        "PATCH",
        &format!("/api/v1/strands/{strand}/inference"),
        Some(serde_json::json!({"provider": "alternate", "reasoning_effort": "high"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reading}");
    assert_eq!(reading["effective"]["provider"], "alternate");
    assert_eq!(reading["effective"]["model"], "soul-model");
    assert_eq!(reading["effective"]["source"]["provider"], "strand");
    assert_eq!(reading["effective"]["source"]["reasoning_effort"], "strand");

    let (status, reading) = call(
        &app,
        "PATCH",
        &format!("/api/v1/strands/{strand}/inference"),
        Some(serde_json::json!({"provider": null})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{reading}");
    assert_eq!(reading["declaration"].get("provider"), None);
    assert_eq!(reading["effective"]["source"]["provider"], "service");
    assert_eq!(reading["effective"]["provider"], "test");

    for body in [
        serde_json::json!({"unknown": "x"}),
        serde_json::json!({"model": ""}),
        serde_json::json!({"provider": "missing"}),
    ] {
        let (status, _) = call(
            &app,
            "PATCH",
            &format!("/api/v1/strands/{strand}/inference"),
            Some(body),
        )
        .await;
        assert!(status.is_client_error(), "{status}");
    }
    let (_, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{strand}/inference"),
        None,
    )
    .await;
    assert_eq!(reading["declaration"]["reasoning_effort"], "high");
    assert_eq!(reading["declaration"].get("model"), None);

    let endpoint = format!("/api/v1/strands/{strand}/inference");
    let first = call(
        &app,
        "PATCH",
        &endpoint,
        Some(serde_json::json!({"provider": "alternate"})),
    );
    let second = call(
        &app,
        "PATCH",
        &endpoint,
        Some(serde_json::json!({"model": "concurrent-model"})),
    );
    let ((first, _), (second, _)) = tokio::join!(first, second);
    assert_eq!(first, StatusCode::OK);
    assert_eq!(second, StatusCode::OK);
    let (_, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{strand}/inference"),
        None,
    )
    .await;
    assert_eq!(reading["declaration"]["provider"], "alternate");
    assert_eq!(reading["declaration"]["model"], "concurrent-model");

    let (status, forked) = call(
        &app,
        "POST",
        &format!("/api/v1/strands/{strand}/fork"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{forked}");
    let child = forked["strand"]["id"].as_str().unwrap();
    let (_, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{child}/inference"),
        None,
    )
    .await;
    assert_eq!(reading["declaration"]["provider"], "alternate");
    assert_eq!(reading["declaration"]["model"], "concurrent-model");

    let (status, forked) = call(
        &app,
        "POST",
        &format!("/api/v1/strands/{strand}/fork"),
        Some(serde_json::json!({"inference": {}})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{forked}");
    let child = forked["strand"]["id"].as_str().unwrap();
    let (_, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{child}/inference"),
        None,
    )
    .await;
    assert_eq!(reading["declaration"], serde_json::json!({}));
    assert_eq!(reading["effective"]["model"], "soul-model");

    let (status, _) = call(&app, "GET", "/api/v1/strands/missing/inference", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &app,
        "PATCH",
        "/api/v1/strands/missing/inference",
        Some(serde_json::json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
