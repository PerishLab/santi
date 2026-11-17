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
    summary = "Fork a complete history prefix, excluding in-flight or failed turns. Optional at selects an exact safe historical sequence; an unavailable position is refused with valid-position guidance. Without at the existing head checkpoint is selected. strand.fork reports the inherited sequence; workspace=current means files are copied as they exist now, not as of that sequence. Omit inference to copy the parent strand's own declaration by value; an explicit inference object replaces the child's declaration, and an empty object inherits from the shared soul.",
    params(("strand" = String, Path)),
    request_body = Option<strand::Fork>,
    responses(
        (status = 200, body = strand::Forked),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn fork() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(strand): Path<String>,
         request: axum::extract::Request| async move {
            let selected: Option<strand::Fork> = super::optional(request).await?;
            Drive(service)
                .fork(strand, selected.unwrap_or_default())
                .await
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
    async fn fork(
        &self,
        strand: String,
        request: strand::Fork,
    ) -> Result<Json<strand::Forked>, ApiError> {
        self.0
            .branching(&strand, request.at, request.inference.as_ref())
            .await
            .map(Json)
            .map_err(super::inference::reject)
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

#[utoipa::path(
    delete,
    path = "/api/v1/strands/{strand}",
    params(("strand" = String, Path)),
    summary = "Retire one idle strand. Pending input, unfinished turns or receipts, active or unacknowledged Jobs, unsettled effects and live forks prevent retirement. Estate commit precedes best-effort file cleanup; startup sweep recovers leftover bytes. Physical collection follows Keel retention.",
    responses(
        (status = 204, description = "Strand ended"),
        (status = 404, body = Fault),
        (status = 409, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn end_strand(
    State(service): State<Service>,
    Path(strand): Path<String>,
) -> Result<StatusCode, ApiError> {
    service
        .end_strand(&strand)
        .await
        .map_err(ApiError::from_service)?;
    Ok(StatusCode::NO_CONTENT)
}
