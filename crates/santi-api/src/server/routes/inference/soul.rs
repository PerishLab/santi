use super::*;
use santi_core::inference::{Patch, Reading};
use santi_estate::Target;

#[utoipa::path(
    get,
    operation_id = "soul",
    tag = "super::routes::inference",
    path = "/api/v1/souls/{soul}/inference",
    summary = "Read a soul's own inference declaration and the effective next-turn values, including the source of each field. An unresolvable current selection returns the declaration with effective:null and an error.",
    params(("soul" = String, Path)),
    responses(
        (status = 200, body = Reading),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(crate) fn read() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(soul): Path<String>| async move {
            match service.inference(Target::Soul, &soul).await {
                Ok(Some(reading)) => Ok(Json(reading)),
                Ok(None) => Err(ApiError::not_found("soul not found")),
                Err(error) => Err(ApiError::from_service(error)),
            }
        },
    )
}

#[utoipa::path(
    patch,
    operation_id = "declare_soul",
    tag = "super::routes::inference",
    path = "/api/v1/souls/{soul}/inference",
    summary = "Apply a partial soul inference declaration. Missing fields keep their current value, null clears a field, and rejection leaves the declaration unchanged. The change applies to later turns, not history or an active turn.",
    params(("soul" = String, Path)),
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
        |State(service): State<Service>, Path(soul): Path<String>, Json(patch): Json<Patch>| async move {
            service
                .declare(Target::Soul, &soul, &patch)
                .await
                .map(Json)
                .map_err(super::reject)
        },
    )
}
