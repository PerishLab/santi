use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    routing::{self, MethodRouter},
};
use santi_core::{
    Fault, job,
    service::{JobDraft, JobRead, Service},
};

use super::ApiError;

const CAPABILITY: &str = "x-santi-job-capability";
const SOUL: &str = "x-santi-soul-id";

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct CreateJobRequest {
    pub description: String,
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_seconds: Option<u64>,
    pub output_limit_bytes: Option<u64>,
    pub remind_every_seconds: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Params {
    stream: Option<job::Stream>,
    cursor: Option<String>,
    limit: Option<usize>,
}

#[utoipa::path(
    post,
    path = "/api/v1/jobs",
    request_body = CreateJobRequest,
    responses(
        (status = 202, body = job::Accepted),
        (status = 400, body = Fault),
        (status = 401, body = Fault),
        (status = 409, body = Fault),
        (status = 503, body = Fault)
    )
)]
pub fn create() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         headers: HeaderMap,
         Json(request): Json<CreateJobRequest>| async move {
            Jobs(service).create(headers, request).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/jobs",
    responses(
        (status = 200, body = [job::Job]),
        (status = 401, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn list() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, headers: HeaderMap| async move {
            Jobs(service).list(headers).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/jobs/{job}",
    params(("job" = String, Path)),
    responses(
        (status = 200, body = job::Job),
        (status = 401, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn get() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, headers: HeaderMap, Path(id): Path<String>| async move {
            Jobs(service).get(headers, id).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/jobs/{job}/cancel",
    params(("job" = String, Path)),
    responses(
        (status = 200, body = job::Job),
        (status = 401, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn cancel() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, headers: HeaderMap, Path(id): Path<String>| async move {
            Jobs(service).cancel(headers, id).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/jobs/{job}/ack",
    params(("job" = String, Path)),
    responses(
        (status = 200, body = job::Job),
        (status = 400, body = Fault),
        (status = 401, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn acknowledge() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, headers: HeaderMap, Path(id): Path<String>| async move {
            Jobs(service).acknowledge(headers, id).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/jobs/{job}/logs",
    params(
        ("job" = String, Path),
        ("stream" = Option<job::Stream>, Query),
        ("cursor" = Option<String>, Query),
        ("limit" = Option<usize>, Query)
    ),
    responses(
        (status = 200, body = job::Log),
        (status = 400, body = Fault),
        (status = 401, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn logs() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>,
         headers: HeaderMap,
         Path(id): Path<String>,
         Query(query): Query<Params>| async move {
            Jobs(service).logs(headers, id, query).await
        },
    )
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub struct Jobs(pub Service);

impl Jobs {
    pub async fn create(
        &self,
        headers: HeaderMap,
        request: CreateJobRequest,
    ) -> Result<(StatusCode, Json<job::Accepted>), ApiError> {
        let capability = header(&headers, CAPABILITY)
            .ok_or_else(|| ApiError::unauthorized("missing job create capability"))?;
        self.0
            .spawn(
                capability,
                JobDraft {
                    description: request.description,
                    command: request.command,
                    cwd: request.cwd,
                    timeout: request.timeout_seconds,
                    output: request.output_limit_bytes,
                    remind: request.remind_every_seconds,
                },
            )
            .await
            .map(|accepted| (StatusCode::ACCEPTED, Json(accepted)))
            .map_err(ApiError::from_service)
    }

    pub async fn list(&self, headers: HeaderMap) -> Result<Json<Vec<job::Job>>, ApiError> {
        let soul =
            header(&headers, SOUL).ok_or_else(|| ApiError::unauthorized("missing soul id"))?;
        self.0
            .jobs(soul)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    pub async fn get(&self, headers: HeaderMap, id: String) -> Result<Json<job::Job>, ApiError> {
        let soul =
            header(&headers, SOUL).ok_or_else(|| ApiError::unauthorized("missing soul id"))?;
        self.0
            .job(soul, &id)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("job not found"))
    }

    pub async fn cancel(&self, headers: HeaderMap, id: String) -> Result<Json<job::Job>, ApiError> {
        let soul =
            header(&headers, SOUL).ok_or_else(|| ApiError::unauthorized("missing soul id"))?;
        self.0
            .cancel(soul, &id)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("job not found"))
    }

    pub async fn acknowledge(
        &self,
        headers: HeaderMap,
        id: String,
    ) -> Result<Json<job::Job>, ApiError> {
        let soul =
            header(&headers, SOUL).ok_or_else(|| ApiError::unauthorized("missing soul id"))?;
        self.0
            .ack(soul, &id)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("job not found"))
    }

    pub async fn logs(
        &self,
        headers: HeaderMap,
        id: String,
        query: Params,
    ) -> Result<Json<job::Log>, ApiError> {
        let soul =
            header(&headers, SOUL).ok_or_else(|| ApiError::unauthorized("missing soul id"))?;
        self.0
            .logs(JobRead {
                soul,
                id: &id,
                stream: query.stream.unwrap_or(job::Stream::Stdout),
                cursor: query.cursor.as_deref().unwrap_or("0"),
                limit: query.limit.unwrap_or(64 * 1024),
            })
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("job not found"))
    }
}
