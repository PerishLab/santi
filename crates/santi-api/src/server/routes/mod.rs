use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{self, MethodRouter, get, post},
};
use santi_core::service::Service;
use santi_core::{
    Fault, Health, downstream::Credential, downstream::Draft, drive::Response, soul::Soul,
    strand::Strand,
};
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};

use super::{
    ApiError,
    errors::{errors, stranded},
    ingress::ingest_webhook,
    sse::{strand_events, transitions, turn_event_stream},
};

pub(super) mod materials;

pub(super) fn router(service: Service) -> Router {
    let api = Router::new()
        .route("/api/v1/openapi.json", get(openapi))
        .route("/api/v1/materials", get(materials::catalog))
        .route("/api/v1/souls/{soul}/materials", get(materials::soul))
        .route("/api/v1/alarms", alarms::set().merge(alarms::list()))
        .route("/api/v1/alarms/{alarm}", alarms::get().merge(alarms::end()))
        .route("/api/v1/alarms/{alarm}/fire", alarms::fire())
        .route("/api/v1/strands", weave().merge(strands()))
        .route("/api/v1/souls", awaken().merge(souls()))
        .route("/api/v1/souls/{soul}", get_soul())
        .route("/api/v1/souls/{soul}/active-turns", get(running))
        .route("/api/v1/souls/{soul}/strands", seat())
        .route(
            "/api/v1/souls/{soul}/environment",
            set_soul_environ().merge(soul_environs()),
        )
        .route(
            "/api/v1/souls/{soul}/environment/{name}",
            end_soul_environ(),
        )
        .route("/api/v1/webhooks", subscribe().merge(webhooks()))
        .route(
            "/api/v1/jobs",
            super::jobs::create().merge(super::jobs::list()),
        )
        .route("/api/v1/jobs/{job}", super::jobs::get())
        .route("/api/v1/jobs/{job}/cancel", super::jobs::cancel())
        .route("/api/v1/jobs/{job}/logs", super::jobs::logs())
        .route("/api/v1/jobs/{job}/ack", super::jobs::acknowledge())
        .route(
            "/api/v1/strands/{strand}",
            get_strand().merge(routing::delete(end_strand)),
        )
        .route(
            "/api/v1/strands/{strand}/environment",
            set_strand_environ().merge(strand_environs()),
        )
        .route(
            "/api/v1/strands/{strand}/environment/{name}",
            end_strand_environ(),
        )
        .route("/api/v1/strands/{strand}/messages", list_messages())
        .route(
            "/api/v1/strands/{strand}/materials",
            material().merge(get(materials::strand)),
        )
        .route("/api/v1/strands/{strand}/events", get(strand_events))
        .route("/api/v1/strands/{strand}/send", post(send))
        .route("/api/v1/strands/{strand}/drive", post(drive))
        .route("/api/v1/strands/{strand}/fork", fork())
        .route("/api/v1/strands/{strand}/compact", exec())
        .route("/api/v1/strands/{strand}/budget", audit())
        .route("/api/v1/strands/{strand}/errors", get(stranded))
        .route("/api/v1/errors/events", get(transitions))
        .route("/api/v1/errors/{scope_kind}/{scope_id}", get(errors))
        .route("/api/v1/receipts/{inbox}", receipt())
        .route("/api/v1/effects/{effect}", get(super::effects::effect))
        .route("/api/v1/effects/{effect}/trace", get(super::effects::trail))
        .route(
            "/api/v1/effects/{effect}/resolve",
            post(super::effects::settle),
        )
        .route("/api/v1/compacts/{compact}", page())
        .route("/api/v1/strands/{strand}/runtime", snapshot())
        .route("/api/v1/strands/{strand}/execution-tail", execution_tail())
        .route("/api/v1/turn-events", get(turn_events))
        .route("/api/v1/turn-events/stream", get(turn_event_stream))
        .route("/api/v1/turns/{turn}/stop", post(stop))
        .route("/api/v1/downstreams", post(enroll).get(downstreams))
        .route("/api/v1/ingest", post(ingest))
        .route(
            "/api/v1/bucket/{soul}/{strand}/{*key}",
            get(crate::bucket::fetch),
        );

    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/webhooks/{name}", post(ingest_webhook))
        .merge(api)
        .route("/api/{*rest}", axum::routing::any(missing))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(service)
}

async fn missing() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "unknown api path"})),
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/health",
    responses(
        (status = 200, body = Health),
        (status = 503, body = Health)
    )
)]
pub async fn health(State(service): State<Service>) -> impl IntoResponse {
    let incidents = service.strained().await;
    let degraded = service.degraded() || incidents > 0;
    let status = if degraded {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::OK
    };
    (
        status,
        Json(Health {
            ok: !degraded,
            degraded,
            service: "santi-api".to_string(),
            incidents: i64::try_from(incidents).unwrap_or(i64::MAX),
        }),
    )
}

mod active;
pub(super) mod alarms;
mod downstream;
mod drive;
mod environment;
mod receipts;
mod turn;
pub use active::*;
pub use downstream::*;
pub use drive::*;
pub use environment::*;
pub use receipts::*;
pub use turn::*;
