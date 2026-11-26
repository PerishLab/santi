use super::*;
use santi_core::event;
use santi_core::turn;

#[utoipa::path(
    get,
    path = "/api/v1/turn-events",
    operation_id = "turn_events",
    security(("downstream_bearer" = [])),
    params(
        ("since" = Option<i64>, Query),
        ("limit" = Option<usize>, Query)
    ),
    responses(
        (status = 200, body = event::Batch),
        (status = 401, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) async fn events(
    State(service): State<Service>,
    headers: axum::http::HeaderMap,
    Query(params): Query<Polling>,
) -> Result<Json<event::Batch>, Problem> {
    let principal = service
        .principal(bearer(&headers))
        .await
        .map_err(Problem::service)?
        .ok_or_else(|| Problem::unauthorized("invalid or missing credential"))?;
    let since = params.since.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(256).clamp(1, 1000);
    service
        .since(since, &principal.prefix, limit)
        .await
        .map(Json)
        .map_err(Problem::service)
}

#[derive(serde::Deserialize)]
pub(super) struct Polling {
    since: Option<i64>,
    limit: Option<usize>,
}

#[utoipa::path(
    post,
    path = "/api/v1/turns/{turn}/stop",
    params(("turn" = String, Path)),
    responses(
        (status = 200, body = turn::Stop),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn stop(
    State(service): State<Service>,
    Path(turn): Path<String>,
) -> Result<Json<turn::Stop>, Problem> {
    service
        .stop(&turn)
        .await
        .map_err(Problem::service)?
        .map(Json)
        .ok_or_else(|| Problem::missing("turn not found"))
}

#[utoipa::path(
    get,
    path = "/api/v1/turns/{turn}",
    params(("turn" = String, Path)),
    responses(
        (status = 200, body = turn::Turn),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn show(
    State(service): State<Service>,
    Path(turn): Path<String>,
) -> Result<Json<turn::Turn>, Problem> {
    service
        .turn(&turn)
        .await
        .map_err(Problem::service)?
        .map(Json)
        .ok_or_else(|| Problem::missing("turn not found"))
}

#[cfg(test)]
mod support;
#[cfg(test)]
mod tests;
