use super::*;
use santi_core::inference::{Patch, Reading};
use santi_estate::Target;

#[utoipa::path(
    get,
    operation_id = "strand",
    tag = "super::routes::inference",
    path = "/api/v1/strands/{strand}/inference",
    summary = "Read a strand's own inference declaration and the effective next-turn values, including the source of each field. History and an active turn keep their recorded configuration. An unresolvable current selection returns effective:null with an error.",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = Reading),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(crate) fn read() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            match service.inference(Target::Strand, &strand).await {
                Ok(Some(reading)) => Ok(Json(reading)),
                Ok(None) => Err(ApiError::not_found("strand not found")),
                Err(error) => Err(ApiError::from_service(error)),
            }
        },
    )
}

#[utoipa::path(
    patch,
    operation_id = "declare_strand",
    tag = "super::routes::inference",
    path = "/api/v1/strands/{strand}/inference",
    summary = "Apply a partial strand inference declaration. Missing fields keep their current value, null clears a field, and rejection leaves the declaration unchanged. The change applies to later turns, not history or an active turn.",
    params(("strand" = String, Path)),
    request_body = Patch,
    responses(
        (status = 200, body = Reading),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(crate) fn declare() -> MethodRouter<Service> {
    routing::patch(
        |State(service): State<Service>, Path(strand): Path<String>, Json(patch): Json<Patch>| async move {
            service
                .declare(Target::Strand, &strand, &patch)
                .await
                .map(Json)
                .map_err(super::reject)
        },
    )
}
