use super::*;

#[utoipa::path(get, path = "/api/v1/souls/{soul}/materials", params(("soul" = String, Path), material::Query),
    summary = "Query own Soul bindings by default; available discovers configured sources, operation retrieves a committed change, reference/blob inspect a pinned revision or an unbound source. No load occurs.",
    responses((status = 200, body = material::View), (status = 404, body = santi_core::Fault)))]
pub(in crate::server) async fn query(
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
#[utoipa::path(post, path = "/api/v1/souls/{soul}/materials/load", params(("soul" = String, Path)),
    request_body = material::Command, summary = "Resolve and pin a complete material revision. Effective next provider round. Reuse operation only for the same payload; query it after uncertain transport.",
    responses((status = 200, body = material::Applied), (status = 400, body = santi_core::Fault), (status = 409, body = santi_core::Fault)))]
pub(in crate::server) async fn load(
    State(service): State<Service>,
    Path(owner): Path<String>,
    Json(command): Json<material::Command>,
) -> Result<Json<material::Applied>, ApiError> {
    service
        .load(Scope::Soul, &owner, command)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}
#[utoipa::path(post, path = "/api/v1/souls/{soul}/materials/drop", params(("soul" = String, Path)),
    request_body = material::Command, summary = "Remove this scope binding; sources and recorded requests remain. Effective next provider round. Reuse operation only for the same payload; query it after uncertain transport.",
    responses((status = 200, body = material::Applied), (status = 400, body = santi_core::Fault), (status = 409, body = santi_core::Fault)))]
pub(in crate::server) async fn drop(
    State(service): State<Service>,
    Path(owner): Path<String>,
    Json(command): Json<material::Command>,
) -> Result<Json<material::Applied>, ApiError> {
    service
        .drop(Scope::Soul, &owner, command)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}
