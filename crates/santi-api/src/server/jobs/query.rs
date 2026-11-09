use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::HeaderMap,
    routing::{self, MethodRouter},
};
use santi_core::{
    Fault,
    job::{Query, QueryError, QueryRequest},
    service::Service,
};

use crate::server::ApiError;

const SOUL: &str = "x-santi-soul-id";

#[utoipa::path(
    post,
    path = "/api/v1/query",
    request_body = QueryRequest,
    params(("x-santi-soul-id" = String, Header, description = "Soul scope supplied by the CLI from --soul/SANTI_SOUL_ID")),
    responses(
        (status = 200, body = Query),
        (status = 400, body = Fault),
        (status = 401, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn query() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         headers: HeaderMap,
         payload: Result<Json<QueryRequest>, JsonRejection>| async move {
            let Json(request) = payload.map_err(|rejection| {
                ApiError::bad_request(format!("invalid query request: {}", rejection.body_text()))
            })?;
            let Some(soul) = headers
                .get(SOUL)
                .and_then(|value| value.to_str().ok())
                .filter(|value| !value.is_empty())
            else {
                return Err(ApiError::unauthorized("missing x-santi-soul-id header"));
            };
            match service.job_query(soul, request).await {
                Ok(query) => Ok(Json(query)),
                Err(QueryError::Invalid(message)) => Err(ApiError::bad_request(message)),
                Err(QueryError::Failed(message)) => Err(ApiError::internal(message)),
            }
        },
    )
}

#[cfg(test)]
mod tests;
