use super::*;
use santi_core::{budget, compact, strand, stream};

mod retire;
pub use retire::*;

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
) -> Result<Json<strand::Posted>, Problem> {
    service
        .send(&strand, request)
        .await
        .map(Json)
        .map_err(Problem::santi)
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
) -> Result<Json<drive::Response>, Problem> {
    service
        .drive(&strand)
        .await
        .map(Json)
        .map_err(|error| Problem::santi(*error))
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
         Query(params): Query<Paging>| async move { Drive(service).page(compact, params).await },
    )
}

#[derive(serde::Deserialize)]
pub(super) struct Paging {
    keyword: Option<String>,
    #[serde(rename = "page_index")]
    index: Option<i64>,
    #[serde(rename = "page_size")]
    size: Option<i64>,
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
    operation_id = "execution_tail",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = stream::Tail),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn tail() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            Drive(service).tail(strand).await
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
    ) -> Result<Json<strand::Forked>, Problem> {
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
    ) -> Result<Json<compact::Report>, Problem> {
        self.0
            .exec(&strand, request)
            .await
            .map(Json)
            .map_err(Problem::service)
    }

    async fn page(&self, compact: String, params: Paging) -> Result<Json<compact::Page>, Problem> {
        self.0
            .page(
                &compact,
                params.keyword.as_deref(),
                params.index.unwrap_or(0),
                params.size.unwrap_or(50),
            )
            .await
            .map_err(Problem::service)?
            .map(Json)
            .ok_or_else(|| Problem::missing("compact not found"))
    }

    async fn snapshot(&self, strand: String) -> Result<Json<stream::Snapshot>, Problem> {
        self.0
            .snapshot(&strand)
            .await
            .map_err(Problem::service)?
            .map(Json)
            .ok_or_else(|| Problem::missing("strand not found"))
    }

    async fn tail(&self, strand: String) -> Result<Json<stream::Tail>, Problem> {
        self.0
            .tail(&strand)
            .await
            .map_err(Problem::service)?
            .map(Json)
            .ok_or_else(|| Problem::missing("strand not found"))
    }

    async fn audit(&self, strand: String) -> Result<Json<budget::Snapshot>, Problem> {
        self.0
            .audit(&strand)
            .await
            .map_err(Problem::service)?
            .map(Json)
            .ok_or_else(|| Problem::missing("strand not found"))
    }
}
