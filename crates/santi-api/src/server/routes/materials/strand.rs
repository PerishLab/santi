use super::*;

#[utoipa::path(get, path = "/api/v1/strands/{strand}/materials", params(("strand" = String, Path), material::Query),
    summary = "Query own Strand bindings and effective selection; effective includes both origins. Context accepts a request or thinking ID (all lists requests), with reference/blob retrieving retained bytes. Available discovers sources. Legacy POST still renders system_prompt/skill without loading.",
    responses((status = 200, body = material::View), (status = 404, body = santi_core::Fault)))]
pub(in crate::server) async fn query(
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
#[utoipa::path(post, path = "/api/v1/strands/{strand}/materials/load", params(("strand" = String, Path)),
    request_body = material::Command, summary = "Resolve and pin a complete material revision. Effective next provider round. Reuse operation only for the same payload; query it after uncertain transport.",
    responses((status = 200, body = material::Applied), (status = 400, body = santi_core::Fault), (status = 409, body = santi_core::Fault)))]
pub(in crate::server) async fn load(
    State(service): State<Service>,
    Path(owner): Path<String>,
    Json(command): Json<material::Command>,
) -> Result<Json<material::Applied>, ApiError> {
    service
        .load(Scope::Strand, &owner, command)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}
#[utoipa::path(post, path = "/api/v1/strands/{strand}/materials/drop", params(("strand" = String, Path)),
    request_body = material::Command, summary = "Remove this scope binding; sources and recorded requests remain. Effective next provider round. Reuse operation only for the same payload; query it after uncertain transport.",
    responses((status = 200, body = material::Applied), (status = 400, body = santi_core::Fault), (status = 409, body = santi_core::Fault)))]
pub(in crate::server) async fn drop(
    State(service): State<Service>,
    Path(owner): Path<String>,
    Json(command): Json<material::Command>,
) -> Result<Json<material::Applied>, ApiError> {
    service
        .drop(Scope::Strand, &owner, command)
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}
