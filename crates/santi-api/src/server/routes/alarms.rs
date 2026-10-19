use super::super::ApiError;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use santi_core::{alarm, ingest, service::Service};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct Owner {
    soul: String,
}

#[utoipa::path(post, path = "/api/v1/alarms", request_body = alarm::Draft, responses((status = 200, body = alarm::View)))]
pub(super) async fn set(
    State(service): State<Service>,
    Json(draft): Json<alarm::Draft>,
) -> Result<Json<alarm::View>, ApiError> {
    service
        .set_alarm(draft)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}

#[utoipa::path(get, path = "/api/v1/alarms", params(("soul" = String, Query)), responses((status = 200, body = alarm::View)))]
pub(super) async fn list(
    State(service): State<Service>,
    Query(owner): Query<Owner>,
) -> Result<Json<alarm::View>, ApiError> {
    service
        .alarms(&owner.soul)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}

#[utoipa::path(get, path = "/api/v1/alarms/{alarm}", params(("alarm" = String, Path), ("soul" = String, Query)), responses((status = 200, body = alarm::View)))]
pub(super) async fn get(
    State(service): State<Service>,
    Path(tag): Path<String>,
    Query(owner): Query<Owner>,
) -> Result<Json<alarm::View>, ApiError> {
    let mut view = service
        .alarms(&owner.soul)
        .await
        .map_err(ApiError::from_service)?;
    view.alarms.retain(|alarm| alarm.id == tag);
    if view.alarms.is_empty() {
        return Err(ApiError::not_found("alarm not found"));
    }
    Ok(Json(view))
}

#[utoipa::path(delete, path = "/api/v1/alarms/{alarm}", params(("alarm" = String, Path), ("soul" = String, Query)), responses((status = 200, body = alarm::View)))]
pub(super) async fn end(
    State(service): State<Service>,
    Path(tag): Path<String>,
    Query(owner): Query<Owner>,
) -> Result<Json<alarm::View>, ApiError> {
    service
        .end_alarm(&tag, &owner.soul)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}

#[utoipa::path(post, path = "/api/v1/alarms/{alarm}/fire", params(("alarm" = String, Path)), request_body = alarm::Occurrence, responses((status = 202, body = ingest::Receipt)))]
pub(super) async fn fire(
    State(service): State<Service>,
    Path(tag): Path<String>,
    Json(occurrence): Json<alarm::Occurrence>,
) -> Result<(axum::http::StatusCode, Json<ingest::Receipt>), ApiError> {
    service
        .fire_alarm(&tag, occurrence)
        .await
        .map(|receipt| (axum::http::StatusCode::ACCEPTED, Json(receipt)))
        .map_err(ApiError::from_service)
}
