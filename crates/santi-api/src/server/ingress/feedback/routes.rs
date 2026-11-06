use super::{Service, render};
use crate::server::ApiError;
use axum::{
    Json,
    extract::{Path, State},
};
use santi_core::{
    Fault,
    receipt::reply::{Request, Resolution, Status},
};

#[utoipa::path(get, path = "/api/v1/receipts/{inbox}/reply", params(("inbox" = String, Path)),
    summary = "Inspect the receipt-owned reply window. Unknown writes freeze every later automatic or soul write; never replay.",
    responses((status = 200, body = Status), (status = 404, body = Fault), (status = 500, body = Fault)))]
pub(in crate::server) async fn inspect(
    State(service): State<Service>,
    Path(inbox): Path<String>,
) -> Result<Json<Status>, ApiError> {
    service
        .reply(&inbox)
        .await
        .map_err(ApiError::from_service)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("reply window not found"))
}

#[utoipa::path(post, path = "/api/v1/receipts/{inbox}/reply", params(("inbox" = String, Path)), request_body = Request,
    summary = "Claim one exact receipt in the caller strand. Cancels unsent automatic intent; known in-flight writes settle before the same message receives this final reply. Same operation is idempotent; changed payload or a second final operation refuses. Acceptance is durable queuing, not delivery.",
    responses((status = 200, body = Status), (status = 400, body = Fault), (status = 500, body = Fault)))]
pub(in crate::server) async fn reply(
    State(service): State<Service>,
    Path(inbox): Path<String>,
    Json(request): Json<Request>,
) -> Result<Json<Status>, ApiError> {
    if request.text.is_empty() || request.text.len() > 20_000 || request.text.contains('\0') {
        return Err(ApiError::bad_request(
            "reply text must contain 1..20000 bytes and no NUL",
        ));
    }
    let body = render::document(&request.text, false);
    let bytes = serde_json::to_vec(&serde_json::json!({"zh_cn":&body}))
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    if bytes.len() > 30_000 {
        return Err(ApiError::bad_request(
            "encoded Feishu post exceeds 30000 bytes",
        ));
    }
    service
        .claim(&request, &inbox, &body)
        .await
        .map_err(ApiError::from_service)?;
    inspect(State(service), Path(inbox)).await
}

#[utoipa::path(post, path = "/api/v1/receipts/{inbox}/reply/resolve", params(("inbox" = String, Path)), request_body = Resolution,
    summary = "Explicit operator attestation that an unknown original platform write has settled remotely. A current message body alone cannot rule out a late write. Does not query or replay the platform; retains original result.",
    responses((status = 200, body = Status), (status = 400, body = Fault), (status = 500, body = Fault)))]
pub(in crate::server) async fn resolve(
    State(service): State<Service>,
    Path(inbox): Path<String>,
    Json(resolution): Json<Resolution>,
) -> Result<Json<Status>, ApiError> {
    service
        .resolution(&inbox, &resolution)
        .await
        .map_err(ApiError::from_service)?;
    inspect(State(service), Path(inbox)).await
}
