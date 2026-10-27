use super::ApiError;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use santi_core::{environ::Scope, material, service::Service};

#[utoipa::path(get, path = "/api/v1/materials", params(material::Query),
    summary = "Discover configured material metadata, or inspect one reference/blob. No binding or instruction change. Content revision excludes paths and acquisition time; provenance is source-declared.",
    responses((status = 200, body = material::View), (status = 400, body = santi_core::Fault)))]
pub(super) async fn catalog(
    State(service): State<Service>,
    Query(mut query): Query<material::Query>,
) -> Result<Json<material::View>, ApiError> {
    query.available = true;
    service
        .catalog(query)
        .map(Json)
        .map_err(ApiError::from_service)
}

#[utoipa::path(get, path = "/api/v1/souls/{soul}/materials", params(("soul" = String, Path), material::Query),
    summary = "Inspect available material metadata or a reference for an existing soul; no load occurs.",
    responses((status = 200, body = material::View), (status = 404, body = santi_core::Fault)))]
pub(super) async fn soul(
    State(service): State<Service>,
    Path(owner): Path<String>,
    Query(query): Query<material::Query>,
) -> Result<Json<material::View>, ApiError> {
    service
        .browse(Scope::Soul, &owner, query)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}

#[utoipa::path(get, path = "/api/v1/strands/{strand}/materials", params(("strand" = String, Path), material::Query),
    summary = "Inspect available material metadata or a reference for an existing strand. Legacy POST still renders current system_prompt/skill without loading.",
    responses((status = 200, body = material::View), (status = 404, body = santi_core::Fault)))]
pub(super) async fn strand(
    State(service): State<Service>,
    Path(owner): Path<String>,
    Query(query): Query<material::Query>,
) -> Result<Json<material::View>, ApiError> {
    service
        .browse(Scope::Strand, &owner, query)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}

#[cfg(test)]
#[path = "materials/tests.rs"]
mod tests;
