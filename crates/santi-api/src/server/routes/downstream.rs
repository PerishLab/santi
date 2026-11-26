use axum::http::HeaderMap;
use santi_core::service::Admission;

use super::*;
use santi_core::ingest;

#[utoipa::path(
    post,
    path = "/api/v1/downstreams",
    responses(
        (status = 200, body = downstream::Credential),
        (status = 400, body = Fault),
        (status = 409, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) async fn enroll(
    State(service): State<Service>,
    Json(request): Json<downstream::Draft>,
) -> Result<Json<downstream::Credential>, Problem> {
    service
        .enroll(request)
        .await
        .map(Json)
        .map_err(Problem::service)
}

#[utoipa::path(
    get,
    path = "/api/v1/downstreams",
    responses(
        (status = 200, body = Vec<downstream::Credential>),
        (status = 500, body = Fault)
    )
)]
pub(super) async fn downstreams(
    State(service): State<Service>,
) -> Result<Json<Vec<downstream::Credential>>, Problem> {
    service
        .downstreams()
        .await
        .map(Json)
        .map_err(Problem::service)
}

#[utoipa::path(
    post,
    path = "/api/v1/ingest",
    security(("downstream_bearer" = [])),
    request_body = ingest::Request,
    responses(
        (status = 202, body = ingest::Receipt),
        (status = 400, body = Fault),
        (status = 401, body = Fault),
        (status = 403, body = Fault),
        (status = 409, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) async fn ingest(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(request): Json<ingest::Request>,
) -> Result<Json<ingest::Receipt>, Problem> {
    let token = bearer(&headers);
    match service
        .downstream(token, request)
        .await
        .map_err(Problem::service)?
    {
        Admission::Accepted(ingest::Outcome::Accepted { receipt }) => Ok(Json(receipt)),
        Admission::Accepted(ingest::Outcome::Rejected { error }) => Err(Problem::santi(*error)),
        Admission::Denied => Err(Problem::unauthorized("invalid or missing credential")),
        Admission::Forbidden => Err(Problem::forbidden(
            "label outside the credential's authorized prefix",
        )),
    }
}

pub(crate) fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("")
}
