use super::*;
use axum::http::HeaderMap;
use santi_core::sources::{
    Record,
    selection::{Choice, Directory},
};
use sha2::digest::CtOutput;
use sha2::{Digest, Sha256};

pub(super) async fn operator(headers: &HeaderMap) -> Result<(), ApiError> {
    let forbidden = || ApiError::forbidden("catalog operator credential required");
    let path = crate::runtime::operator().ok_or_else(forbidden)?;
    let value = headers.get("x-santi-catalog-token").ok_or_else(forbidden)?;
    let secret = tokio::fs::read_to_string(path)
        .await
        .map_err(|_| forbidden())?;
    let secret = secret.trim_end();
    let expected = CtOutput::<Sha256>::new(Sha256::digest(secret.as_bytes()));
    let supplied = CtOutput::<Sha256>::new(Sha256::digest(value.as_bytes()));
    if secret.len() < 32 || expected != supplied {
        return Err(forbidden());
    }
    Ok(())
}

#[utoipa::path(get, path="/api/v1/inference/sources", responses((status=200, body=Vec<Record>)))]
pub(crate) async fn list(State(service): State<Service>) -> Result<Json<Vec<Record>>, ApiError> {
    service
        .sources()
        .await
        .map(Json)
        .map_err(ApiError::from_service)
}

#[utoipa::path(get, path="/api/v1/inference/choices/{provider}/{family}/{scenario}", params(("provider"=String,Path),("family"=String,Path),("scenario"=String,Path)), responses((status=200,body=Choice),(status=404,body=Fault)))]
pub(crate) async fn explain(
    State(service): State<Service>,
    Path((provider, family, scenario)): Path<(String, String, String)>,
) -> Result<Json<Choice>, ApiError> {
    let rows = service.sources().await.map_err(ApiError::from_service)?;
    Directory(&rows)
        .explain(&provider, &family, &scenario)
        .map(Json)
        .map_err(ApiError::not_found)
}
