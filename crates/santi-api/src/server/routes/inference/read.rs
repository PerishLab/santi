use super::*;

#[tokio::test]
async fn unresolved() {
    let (_root, app) = serve(Declaration::default(), "https://api.openai.com/v1").await;
    let (status, created) = call(&app, "POST", "/api/v1/strands", None).await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let soul = created["strand"]["soul"].as_str().unwrap();
    let (status, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/souls/{soul}/inference"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reading["declaration"], serde_json::json!({}));
    assert!(reading.get("effective").is_none(), "{reading}");
    assert!(reading["error"].as_str().unwrap().contains("scenario"));
}

#[tokio::test]
async fn admitted() {
    use axum::{Router, routing::post};
    use std::time::Duration;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
    let provider = Router::new().route("/v1/responses", post(|| async {
        ([("content-type","text/event-stream")], concat!(
            "data: {\"type\":\"response.output_text.delta\",\"delta\":\"ok\"}\n\n",
            "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_scene\"}}\n\n",
        ))
    }));
    let task = tokio::spawn(async move { axum::serve(listener, provider).await.unwrap() });
    let (_root, app) = serve(
        Declaration {
            scenario: Some("gateway/family/routine".into()),
            ..Default::default()
        },
        &endpoint,
    )
    .await;
    let (_, created) = call(&app, "POST", "/api/v1/strands", None).await;
    let strand = created["strand"]["id"].as_str().unwrap();
    let (status, reading) = call(
        &app,
        "GET",
        &format!("/api/v1/strands/{strand}/inference"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let effective = &reading["effective"];
    assert_eq!(effective["scenario"], "gateway/family/routine");
    assert_eq!(effective["source"]["scenario"], "service");
    assert_eq!(effective["source"]["route"]["provider"]["name"], "gateway");
    assert!(effective.get("provider").is_none());
    assert!(effective["source"].get("provider").is_none());

    let (status, posted) = call(
        &app,
        "POST",
        &format!("/api/v1/strands/{strand}/send"),
        Some(serde_json::json!({"content":[{"type":"text","text":"fixture"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{posted}");
    let inbox = posted["receipt"]["inbox"].as_str().unwrap();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    let id = loop {
        let (status, receipt) = call(&app, "GET", &format!("/api/v1/receipts/{inbox}"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_ne!(receipt["state"], "failed", "{receipt}");
        if receipt["state"] == "completed" {
            break receipt["transitions"]
                .as_array()
                .unwrap()
                .iter()
                .rev()
                .find_map(|event| event["turn"].as_str())
                .unwrap()
                .to_owned();
        }
        assert!(tokio::time::Instant::now() < deadline, "{receipt}");
        tokio::time::sleep(Duration::from_millis(25)).await;
    };
    let (status, turn) = call(&app, "GET", &format!("/api/v1/turns/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(&turn["inference"], effective);
    task.abort();
}
