use axum::{
    Json,
    extract::{Path, Query, State},
};
use santi_core::service::Service;
use santi_core::{Fault, History, Incident, Scope};

use super::ApiError;

#[derive(serde::Deserialize)]
pub struct ErrorQueryParams {
    pub limit: Option<i64>,
    pub before: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/errors",
    summary = "List retained active and resolved incidents, newest first by first.seen then id descending. Pass the last returned id as before for the next page. Limit defaults to 50 and is clamped to 1..1000. A traversal is not a snapshot; new newer arrivals appear on a fresh first page. Retired records are unavailable; an absent or foreign cursor returns 400.",
    params(
        ("strand" = String, Path),
        ("limit" = Option<i64>, Query),
        ("before" = Option<String>, Query)
    ),
    responses(
        (status = 200, body = Vec<Incident>),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn stranded(
    State(service): State<Service>,
    Path(strand): Path<String>,
    Query(params): Query<ErrorQueryParams>,
) -> Result<Json<Vec<Incident>>, ApiError> {
    service
        .history(
            &Scope::new("strand", &strand),
            params.limit.unwrap_or(50),
            params.before.as_deref(),
        )
        .await
        .map_err(ApiError::from_service)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("strand not found"))
}

#[utoipa::path(
    get,
    path = "/api/v1/errors/{scope_kind}/{scope_id}",
    summary = "List retained scoped incidents with the same ordering and before cursor as strand errors. Missing or unsupported scopes return an empty array. Delivered and resolved incidents remain queryable while their owner lives.",
    params(
        ("scope_kind" = String, Path),
        ("scope_id" = String, Path),
        ("limit" = Option<i64>, Query),
        ("before" = Option<String>, Query)
    ),
    responses(
        (status = 200, body = Vec<Incident>),
        (status = 400, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn errors(
    State(service): State<Service>,
    Path((scope_kind, scope_id)): Path<(String, String)>,
    Query(params): Query<ErrorQueryParams>,
) -> Result<Json<Vec<Incident>>, ApiError> {
    service
        .history(
            &Scope::new(scope_kind, scope_id),
            params.limit.unwrap_or(50),
            params.before.as_deref(),
        )
        .await
        .map(|rows| Json(rows.unwrap_or_default()))
        .map_err(ApiError::from_service)
}

#[utoipa::path(
    get,
    path = "/api/v1/incidents/{incident}",
    summary = "Inspect a retained incident by its exact id, including delivered opened/resolved event snapshots. Repeated reports retain first/latest summaries and a count, not every occurrence. Missing or retired incidents return 404.",
    params(("incident" = String, Path)),
    responses((status = 200, body = History), (status = 404, body = Fault), (status = 500, body = Fault))
)]
pub async fn incident(
    State(service): State<Service>,
    Path(incident): Path<String>,
) -> Result<Json<History>, ApiError> {
    service
        .incident(&incident)
        .await
        .map_err(ApiError::from_service)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("incident not found"))
}
