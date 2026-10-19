use super::super::ApiError;
use axum::{
    Json,
    extract::{Path, Query, State},
    routing::{self, MethodRouter},
};
use santi_core::{alarm, ingest, service::Service};
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct Owner {
    soul: String,
}

struct Alarms(Service);

#[utoipa::path(post, path = "/api/v1/alarms", request_body = alarm::Draft, responses((status = 200, body = alarm::View)))]
pub(super) fn set() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, Json(draft): Json<alarm::Draft>| async move {
            Alarms(service).set(draft).await
        },
    )
}

#[utoipa::path(get, path = "/api/v1/alarms", params(("soul" = String, Query)), responses((status = 200, body = alarm::View)))]
pub(super) fn list() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Query(owner): Query<Owner>| async move {
            Alarms(service).list(owner).await
        },
    )
}

#[utoipa::path(get, path = "/api/v1/alarms/{alarm}", params(("alarm" = String, Path), ("soul" = String, Query)), responses((status = 200, body = alarm::View)))]
pub(super) fn get() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(tag): Path<String>, Query(owner): Query<Owner>| async move {
            Alarms(service).get(tag, owner).await
        },
    )
}

#[utoipa::path(delete, path = "/api/v1/alarms/{alarm}", params(("alarm" = String, Path), ("soul" = String, Query)), responses((status = 200, body = alarm::View)))]
pub(super) fn end() -> MethodRouter<Service> {
    routing::delete(
        |State(service): State<Service>, Path(tag): Path<String>, Query(owner): Query<Owner>| async move {
            Alarms(service).end(tag, owner).await
        },
    )
}

#[utoipa::path(post, path = "/api/v1/alarms/{alarm}/fire", params(("alarm" = String, Path)), request_body = alarm::Occurrence, responses((status = 202, body = ingest::Receipt)))]
pub(super) fn fire() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(tag): Path<String>,
         Json(occurrence): Json<alarm::Occurrence>| async move {
            Alarms(service).fire(tag, occurrence).await
        },
    )
}

impl Alarms {
    async fn set(&self, draft: alarm::Draft) -> Result<Json<alarm::View>, ApiError> {
        self.0
            .set_alarm(draft)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn list(&self, owner: Owner) -> Result<Json<alarm::View>, ApiError> {
        self.0
            .alarms(&owner.soul)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn get(&self, tag: String, owner: Owner) -> Result<Json<alarm::View>, ApiError> {
        let mut view = self
            .0
            .alarms(&owner.soul)
            .await
            .map_err(ApiError::from_service)?;
        view.alarms.retain(|alarm| alarm.id == tag);
        if view.alarms.is_empty() {
            return Err(ApiError::not_found("alarm not found"));
        }
        Ok(Json(view))
    }

    async fn end(&self, tag: String, owner: Owner) -> Result<Json<alarm::View>, ApiError> {
        self.0
            .end_alarm(&tag, &owner.soul)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn fire(
        &self,
        tag: String,
        occurrence: alarm::Occurrence,
    ) -> Result<(axum::http::StatusCode, Json<ingest::Receipt>), ApiError> {
        self.0
            .fire_alarm(&tag, occurrence)
            .await
            .map(|receipt| (axum::http::StatusCode::ACCEPTED, Json(receipt)))
            .map_err(ApiError::from_service)
    }
}
