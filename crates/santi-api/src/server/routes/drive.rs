use super::*;
use santi_core::{budget, compact, strand, stream};

#[utoipa::path(
    post,
    path = "/api/v1/strands/{strand}/send",
    params(("strand" = String, Path)),
    summary = "Persist valid input and acknowledge its durable receipt without waiting for context assembly or strand processing. Follow the receipt for subsequent progress; do not resend accepted input.",
    request_body = strand::Post,
    responses(
        (status = 200, body = strand::Posted),
        (status = 423, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault),
        (status = 503, body = Fault)
    )
)]
pub async fn send(
    State(service): State<Service>,
    Path(strand): Path<String>,
    Json(request): Json<strand::Post>,
) -> Result<Json<strand::Posted>, ApiError> {
    service
        .send(&strand, request)
        .await
        .map(Json)
        .map_err(ApiError::from_santi)
}

#[utoipa::path(
    post,
    path = "/api/v1/strands/{strand}/drive",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = drive::Response),
        (status = 404, body = Fault),
        (status = 423, body = Fault),
        (status = 500, body = Fault),
        (status = 503, body = Fault)
    )
)]
pub async fn drive(
    State(service): State<Service>,
    Path(strand): Path<String>,
) -> Result<Json<drive::Response>, ApiError> {
    service
        .drive(&strand)
        .await
        .map(Json)
        .map_err(|error| ApiError::from_santi(*error))
}

#[utoipa::path(
    post,
    path = "/api/v1/strands/{strand}/fork",
    summary = "Fork a complete history prefix, excluding in-flight or failed turns. strand.fork reports the actual inherited sequence; workspace files are copied as they exist now, not as of that sequence.",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = strand::Forked),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn fork() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            Drive(service).fork(strand).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/strands/{strand}/compact",
    params(("strand" = String, Path)),
    request_body = compact::Exec,
    responses(
        (status = 200, body = compact::Report),
        (status = 400, body = Fault),
        (status = 409, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn exec() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(strand): Path<String>,
         Json(request): Json<compact::Exec>| async move {
            Drive(service).exec(strand, request).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/compacts/{compact}",
    params(
        ("compact" = String, Path),
        ("keyword" = Option<String>, Query),
        ("page_index" = Option<i64>, Query),
        ("page_size" = Option<i64>, Query)
    ),
    responses(
        (status = 200, body = compact::Page),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn page() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>,
         Path(compact): Path<String>,
         Query(params): Query<CompactQueryParams>| async move {
            Drive(service).page(compact, params).await
        },
    )
}

#[derive(serde::Deserialize)]
pub(super) struct CompactQueryParams {
    keyword: Option<String>,
    page_index: Option<i64>,
    page_size: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/runtime",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = stream::Snapshot),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn snapshot() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            Drive(service).snapshot(strand).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/execution-tail",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = stream::ExecutionTail),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn execution_tail() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            Drive(service).execution_tail(strand).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/budget",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = budget::Snapshot),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn audit() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            Drive(service).audit(strand).await
        },
    )
}

pub(super) async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(crate::server::openapi::document())
}

struct Drive(Service);

impl Drive {
    async fn fork(&self, strand: String) -> Result<Json<strand::Forked>, ApiError> {
        self.0
            .fork(&strand)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn exec(
        &self,
        strand: String,
        request: compact::Exec,
    ) -> Result<Json<compact::Report>, ApiError> {
        self.0
            .exec(&strand, request)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn page(
        &self,
        compact: String,
        params: CompactQueryParams,
    ) -> Result<Json<compact::Page>, ApiError> {
        self.0
            .page(
                &compact,
                params.keyword.as_deref(),
                params.page_index.unwrap_or(0),
                params.page_size.unwrap_or(50),
            )
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("compact not found"))
    }

    async fn snapshot(&self, strand: String) -> Result<Json<stream::Snapshot>, ApiError> {
        self.0
            .snapshot(&strand)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("strand not found"))
    }

    async fn execution_tail(
        &self,
        strand: String,
    ) -> Result<Json<stream::ExecutionTail>, ApiError> {
        self.0
            .execution_tail(&strand)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("strand not found"))
    }

    async fn audit(&self, strand: String) -> Result<Json<budget::Snapshot>, ApiError> {
        self.0
            .audit(&strand)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("strand not found"))
    }
}
