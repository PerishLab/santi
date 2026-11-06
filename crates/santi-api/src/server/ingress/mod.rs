mod ack;
pub(super) mod feedback;
mod process;

use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use santi_core::Fault;
use santi_core::service::Service;
use serde_json::json;
use sha2::{Digest as _, Sha256};

use crate::webhook::{WebhookOutcome, adaptor_for};

use super::ApiError;
use santi_core::ingest;

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/{name}",
    summary = "Webhook ingest endpoint. Not bearer-gated — authenticity is established by the\nadaptor verifying the request signature against the subscription's secret. An\nout-of-scope or self-authored event returns 200 without waking the soul.",
    params(("name" = String, Path)),
    request_body(content_type = "application/json", description = "Raw provider event payload"),
    responses(
        (status = 200, description = "Control response or ignored event"),
        (status = 202, body = santi_core::ingest::Receipt),
        (status = 401, body = Fault),
        (status = 423, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault),
        (status = 503, body = Fault)
    )
)]
pub(super) async fn ingest_webhook(
    State(service): State<Service>,
    Path(name): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let subscription = service
        .webhook(&name)
        .await
        .map_err(ApiError::from_service)?
        .ok_or_else(|| ApiError::not_found("webhook not found"))?;
    let soul = service
        .soul(&subscription.soul)
        .await
        .map_err(ApiError::from_service)?
        .ok_or_else(|| ApiError::not_found("soul not found"))?;
    let account = soul
        .identity
        .as_ref()
        .and_then(|identity| identity.channels.get(&subscription.adaptor));
    let adaptor = adaptor_for(&subscription.adaptor, account.map(String::as_str))
        .ok_or_else(|| ApiError::internal(format!("unknown adaptor {}", subscription.adaptor)))?;
    let secret = crate::config::env(&subscription.credential).ok_or_else(|| {
        ApiError::unauthorized(format!(
            "webhook secret env {} is not set",
            subscription.credential
        ))
    })?;
    adaptor
        .verify(&headers, &body, &secret)
        .map_err(ApiError::from_webhook)?;
    let event = match adaptor
        .normalize(&headers, &body, &secret, &name)
        .map_err(ApiError::from_webhook)?
    {
        WebhookOutcome::Reply(reply) => return Ok((StatusCode::OK, Json(reply)).into_response()),
        WebhookOutcome::Event(event) => event,
    };
    if !event.in_scope || event.self_authored {
        return Ok(StatusCode::OK.into_response());
    }
    let label = if subscription.strategy == "single" {
        format!("{}:{}", subscription.adaptor, name)
    } else {
        event.label.clone()
    };
    let reply = feedback::binding(&subscription.adaptor, event.metadata.as_ref());
    let confirmation = ack::message(&subscription.adaptor, event.metadata.as_ref());
    let fingerprint = event
        .metadata
        .as_ref()
        .and_then(|m| m.get("fingerprint"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let source = ingest::Source::new("webhook")
        .with_ref(format!("{}:{name}", subscription.adaptor))
        .with_metadata(json!({
            "subscription": name,
            "feedback": reply.is_some(),
            "adaptor": subscription.adaptor,
            "strategy": subscription.strategy,
            "event_label": event.label,
            "materialized_label": label,
            "event": event.metadata,
        }));
    let outcome = match event.delivery.as_deref() {
        Some(delivery) => {
            let digest =
                fingerprint.unwrap_or_else(|| digest(&subscription.adaptor, &headers, &body));
            let input = santi_core::service::Envelope {
                soul: &subscription.soul,
                label: &label,
                text: event.text,
                source: Some(source),
            };
            let delivery = santi_core::service::Delivery {
                subscription: &subscription.name,
                id: delivery,
                digest: &digest,
            };
            match reply.as_ref() {
                Some(reply) => service.windowed(input, delivery, reply).await,
                None => service.deliver(input, delivery).await,
            }
        }
        None if subscription.adaptor == "github" => {
            return Err(ApiError::bad_request("missing X-GitHub-Delivery header"));
        }
        None => {
            service
                .sourced(&subscription.soul, &label, event.text, Some(source))
                .await
        }
    }
    .map_err(ApiError::from_service)?;
    match outcome {
        ingest::Outcome::Accepted { receipt } => {
            ack::accept(&service, confirmation, &receipt).await;
            Ok((StatusCode::ACCEPTED, Json(receipt)).into_response())
        }
        ingest::Outcome::Rejected { error } => Err(ApiError::from_santi(*error)),
    }
}

fn digest(adaptor: &str, headers: &HeaderMap, body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(adaptor.as_bytes());
    hasher.update([0]);
    if let Some(event) = headers
        .get("X-GitHub-Event")
        .and_then(|value| value.to_str().ok())
    {
        hasher.update(event.as_bytes());
    }
    hasher.update([0]);
    hasher.update(body);
    hex::encode(hasher.finalize())
}

#[utoipa::path(get, path = "/api/v1/receipts/{inbox}/acknowledgement", params(("inbox" = String, Path)),
    summary = "Inspect the separate channel acknowledgement dispatch and retained result. Dispatching after interruption is uncertain; never resend the accepted task to recover its acknowledgement.",
    responses((status = 200, body = santi_core::receipt::Acknowledgement), (status = 404, body = Fault), (status = 500, body = Fault)))]
pub(super) async fn acknowledgement(
    State(service): State<Service>,
    Path(inbox): Path<String>,
) -> Result<Json<santi_core::receipt::Acknowledgement>, ApiError> {
    service
        .acknowledgement(&inbox)
        .await
        .map_err(ApiError::from_service)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("acknowledgement not found"))
}

#[utoipa::path(post, path = "/api/v1/receipts/{inbox}/acknowledgement/confirm", params(("inbox" = String, Path)), request_body = santi_core::receipt::Confirmation,
    summary = "Explicit operator judgment after exact reaction observation. Resolves only an unknown acknowledgement and preserves the original dispatch/result. Does not query the platform or resend any task/reaction. Absence requires sufficient page coverage; evidence is the caller's attestation.",
    responses((status = 200, body = santi_core::receipt::Acknowledgement), (status = 400, body = Fault), (status = 500, body = Fault)))]
pub(super) async fn confirmed(
    State(service): State<Service>,
    Path(inbox): Path<String>,
    Json(resolution): Json<santi_core::receipt::Confirmation>,
) -> Result<Json<santi_core::receipt::Acknowledgement>, ApiError> {
    service
        .confirmed(&inbox, &resolution)
        .await
        .map_err(ApiError::from_service)?;
    acknowledgement(State(service), Path(inbox)).await
}
