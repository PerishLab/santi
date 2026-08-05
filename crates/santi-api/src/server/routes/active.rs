use super::*;
use santi_core::service::interrupt as active;

#[derive(serde::Deserialize)]
pub struct Params {
    exclude: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/souls/{soul}/active-turns",
    params(
        ("soul" = String, Path),
        ("exclude" = Option<String>, Query)
    ),
    responses(
        (status = 200, body = active::Projection),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn running(
    State(service): State<Service>,
    Path(soul): Path<String>,
    Query(params): Query<Params>,
) -> Result<Json<active::Projection>, ApiError> {
    service
        .running(&soul, params.exclude.as_deref())
        .await
        .map_err(ApiError::from_service)?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("soul not found"))
}
