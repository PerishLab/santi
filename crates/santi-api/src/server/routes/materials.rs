use super::ApiError;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use santi_core::{environ::Scope, material, service::Service};

pub(in crate::server) mod soul;
pub(in crate::server) mod strand;

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

#[cfg(test)]
#[path = "materials/tests.rs"]
mod tests;
