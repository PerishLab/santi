#[test]
fn surface() {
    let value: serde_json::Value =
        serde_json::from_str(&santi_api::export_openapi_json().unwrap()).unwrap();
    assert!(value["paths"]["/api/v1/souls/{soul}/wake"].is_null());
    assert!(value["paths"]["/api/v1/alarms"]["post"].is_object());
    assert!(value["paths"]["/api/v1/alarms/{alarm}/fire"]["post"].is_object());
}

#[test]
fn errors() {
    assert_eq!(
        super::status("adapt: alarm generation conflicts; read current declaration"),
        axum::http::StatusCode::CONFLICT
    );
    assert_eq!(
        super::status("adapt: alarm not found or ended"),
        axum::http::StatusCode::NOT_FOUND
    );
    assert_eq!(
        super::status("alarm cron needs five numeric fields"),
        axum::http::StatusCode::BAD_REQUEST
    );
}
