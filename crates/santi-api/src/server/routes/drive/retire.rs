use super::*;

#[utoipa::path(
    delete,
    operation_id = "end_strand",
    path = "/api/v1/strands/{strand}",
    params(("strand" = String, Path)),
    summary = "Retire one idle strand. Pending input, unfinished turns or receipts, active or unacknowledged Jobs, unsettled effects and live forks prevent retirement. Estate commit precedes best-effort file cleanup; startup sweep recovers leftover bytes. Physical collection follows Keel retention.",
    responses(
        (status = 200, body = strand::Retirement),
        (status = 404, body = Fault),
        (status = 409, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn retire(
    State(service): State<Service>,
    Path(strand): Path<String>,
) -> Result<Json<strand::Retirement>, Problem> {
    service
        .retire(&strand)
        .await
        .map(Json)
        .map_err(Problem::service)
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/retirement",
    params(("strand" = String, Path)),
    summary = "Read retained retirement evidence for an exact strand ID.",
    responses(
        (status = 200, body = strand::Retirement),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub async fn retirement(
    State(service): State<Service>,
    Path(strand): Path<String>,
) -> Result<Json<strand::Retirement>, Problem> {
    service
        .retirement(&strand)
        .await
        .map_err(Problem::service)?
        .map(Json)
        .ok_or_else(|| Problem::missing("strand retirement not found"))
}
