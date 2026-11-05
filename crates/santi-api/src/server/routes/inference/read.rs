use super::*;

#[tokio::test]
async fn unresolved() {
    let (_root, app) = serve(Declaration::default()).await;
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
    assert!(reading["error"].as_str().unwrap().contains("provider"));
}
