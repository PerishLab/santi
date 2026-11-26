use super::*;
use axum::http::HeaderMap;
use santi_core::sources::{Kind, Record, explanation::Explanation, selection::Directory};
use sha2::digest::CtOutput;
use sha2::{Digest, Sha256};

pub(super) async fn operator(headers: &HeaderMap) -> Result<(), Problem> {
    let forbidden = || Problem::forbidden("catalog operator credential required");
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
pub(crate) async fn list(State(service): State<Service>) -> Result<Json<Vec<Record>>, Problem> {
    service.sources().await.map(Json).map_err(Problem::service)
}

#[utoipa::path(get, path="/api/v1/inference/choices/{provider}/{family}/{scenario}", params(("provider"=String,Path),("family"=String,Path),("scenario"=String,Path)), responses((status=200,body=Explanation),(status=404,body=Fault)))]
pub(crate) async fn explain(
    State(service): State<Service>,
    Path((provider, family, scenario)): Path<(String, String, String)>,
) -> Result<Json<Explanation>, Problem> {
    let rows = service.sources().await.map_err(Problem::service)?;
    Directory(&rows)
        .describe(&provider, &family, &scenario)
        .map(Json)
        .map_err(Problem::missing)
}

#[utoipa::path(get, path="/api/v1/inference/sources/{kind}/{id}", params(("kind"=Kind,Path),("id"=i64,Path)), responses((status=200,body=Record),(status=404,body=Fault)))]
pub(crate) async fn read(
    State(service): State<Service>,
    Path((kind, id)): Path<(Kind, i64)>,
) -> Result<Json<Record>, Problem> {
    service
        .source(kind, id)
        .await
        .map_err(Problem::service)?
        .map(Json)
        .ok_or_else(|| Problem::missing("inference source not found"))
}
