use std::sync::Arc;

use axum::{Json, extract::Path, extract::State, http::StatusCode};
use santi_api::{control_wake_handler, wake_status_handler};
use santi_core::service::{self, Service};
use santi_core::{soul, wake};

use super::DriverProvider;

#[tokio::test]
async fn controls() {
    assert_eq!(
        super::status("wake lease generation conflicts: expected 1, current 2"),
        StatusCode::CONFLICT
    );
    assert_eq!(
        super::status("wake renew requires generation"),
        StatusCode::BAD_REQUEST
    );
    assert!(
        santi_api::export_openapi_json()
            .expect("openapi")
            .contains("/api/v1/souls/{soul}/wake")
    );
    let temp = tempfile::tempdir().expect("temp dir");
    let database = temp.path().join("santi.sqlite");
    super::support::bootstrap(&database).await;
    let service = Service::open(
        service::Config {
            database: database.display().to_string(),
            runtime: temp.path().join("runtime").display().to_string(),
            execution: temp.path().join("execution").display().to_string(),
            bind: Some("127.0.0.1:0".to_string()),
            constitution: None,
            environment: Default::default(),
        },
        Arc::new(DriverProvider),
    )
    .await
    .expect("open service");
    let soul = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken soul");

    let dormant = success(wake_status_handler(State(service.clone()), Path(soul.id.clone())).await);
    assert_eq!(dormant.state, wake::State::Revoked);
    assert_eq!(dormant.generation, 0);

    let invited = success(
        control_wake_handler(
            State(service.clone()),
            Path(soul.id.clone()),
            Json(wake::CallerRequest {
                action: wake::CallerAction::Enable,
            }),
        )
        .await,
    );
    assert_eq!(invited.state, wake::State::Active);
    assert_eq!(invited.rounds_remaining, wake::ROUNDS);
    let repeated = success(
        control_wake_handler(
            State(service.clone()),
            Path(soul.id.clone()),
            Json(wake::CallerRequest {
                action: wake::CallerAction::Enable,
            }),
        )
        .await,
    );
    assert_eq!(repeated, invited);

    let revoked = success(
        control_wake_handler(
            State(service.clone()),
            Path(soul.id.clone()),
            Json(wake::CallerRequest {
                action: wake::CallerAction::Disable,
            }),
        )
        .await,
    );
    assert_eq!(revoked.state, wake::State::Revoked);
    assert_eq!(revoked.generation, invited.generation + 1);

    let missing = wake_status_handler(State(service), Path("soul_missing".to_string()))
        .await
        .expect_err("missing soul");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

fn success<T>(result: Result<Json<T>, santi_api::ApiError>) -> T {
    match result {
        Ok(Json(value)) => value,
        Err(error) => panic!("{}: {}", error.code(), error.message()),
    }
}
