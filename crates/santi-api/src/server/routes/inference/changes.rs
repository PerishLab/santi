use super::sources::operator;
use super::*;
use axum::http::HeaderMap;
use santi_core::sources::{Amendment, Definition, Record};
#[utoipa::path(post,path="/api/v1/inference/sources",security(("catalog_operator"=[])),request_body=Definition,responses((status=200,body=Record),(status=400,body=Fault),(status=403,body=Fault),(status=409,body=Fault)))]
pub(crate) async fn register(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(definition): Json<Definition>,
) -> Result<Json<Record>, Problem> {
    operator(&headers).await?;
    definition.validate().map_err(Problem::invalid)?;
    service
        .define(definition)
        .await
        .map(Json)
        .map_err(Problem::conflict)
}

#[utoipa::path(patch,path="/api/v1/inference/sources",security(("catalog_operator"=[])),request_body=Amendment,responses((status=200,body=Record),(status=400,body=Fault),(status=403,body=Fault),(status=409,body=Fault)))]
pub(crate) async fn amend(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(edit): Json<Amendment>,
) -> Result<Json<Record>, Problem> {
    operator(&headers).await?;
    edit.replacement.validate().map_err(Problem::invalid)?;
    service
        .amend(edit)
        .await
        .map(Json)
        .map_err(Problem::conflict)
}

#[utoipa::path(delete,path="/api/v1/inference/sources",security(("catalog_operator"=[])),request_body=Record,responses((status=204),(status=400,body=Fault),(status=403,body=Fault),(status=409,body=Fault)))]
pub(crate) async fn withdraw(
    State(service): State<Service>,
    headers: HeaderMap,
    Json(record): Json<Record>,
) -> Result<StatusCode, Problem> {
    operator(&headers).await?;
    service.withdraw(record).await.map_err(Problem::conflict)?;
    Ok(StatusCode::NO_CONTENT)
}
