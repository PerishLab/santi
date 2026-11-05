use super::*;

mod ops;
use ops::Receipts;
use santi_core::render;
use santi_core::{inference, receipt, soul, strand, webhook};

#[utoipa::path(
    get,
    path = "/api/v1/receipts/{inbox}",
    params(("inbox" = String, Path)),
    responses(
        (status = 200, body = receipt::Status),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub fn receipt() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(inbox): Path<String>| async move {
            Receipts(service).receipt(inbox).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/strands",
    request_body = Option<inference::Draft>,
    responses(
        (status = 200, body = strand::Created),
        (status = 400, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn weave() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, request: axum::extract::Request| async move {
            let selected: Option<inference::Draft> = super::optional(request).await?;
            Receipts(service)
                .weave(selected.and_then(|draft| draft.inference).as_ref())
                .await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/souls/{soul}/strands",
    params(("soul" = String, Path)),
    request_body = Option<inference::Draft>,
    responses(
        (status = 200, body = strand::Created),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn seat() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(soul): Path<String>,
         request: axum::extract::Request| async move {
            let selected: Option<inference::Draft> = super::optional(request).await?;
            Receipts(service)
                .seat(soul, selected.and_then(|draft| draft.inference).as_ref())
                .await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/strands",
    responses((status = 200, body = [Strand]), (status = 500, body = Fault))
)]
pub(super) fn strands() -> MethodRouter<Service> {
    routing::get(|State(service): State<Service>| async move { Receipts(service).strands().await })
}

#[utoipa::path(
    post,
    path = "/api/v1/souls",
    request_body = soul::Draft,
    responses(
        (status = 200, body = Soul),
        (status = 400, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn awaken() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, Json(request): Json<soul::Draft>| async move {
            Receipts(service).awaken(request).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/souls",
    responses((status = 200, body = [Soul]), (status = 500, body = Fault))
)]
pub(super) fn souls() -> MethodRouter<Service> {
    routing::get(|State(service): State<Service>| async move { Receipts(service).souls().await })
}

#[utoipa::path(
    get,
    path = "/api/v1/souls/{soul}",
    params(("soul" = String, Path)),
    responses(
        (status = 200, body = Soul),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn get_soul() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(soul): Path<String>| async move {
            Receipts(service).get_soul(soul).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks",
    request_body = webhook::Draft,
    responses(
        (status = 200, body = webhook::Subscription),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 409, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn subscribe() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, Json(request): Json<webhook::Draft>| async move {
            Receipts(service).subscribe(request).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/webhooks",
    responses((status = 200, body = [webhook::Subscription]), (status = 500, body = Fault))
)]
pub(super) fn webhooks() -> MethodRouter<Service> {
    routing::get(|State(service): State<Service>| async move { Receipts(service).webhooks().await })
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = strand::Detail),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn get_strand() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            Receipts(service).get_strand(strand).await
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/strands/{strand}/messages",
    params(("strand" = String, Path)),
    responses(
        (status = 200, body = [santi_core::message::Placed]),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn list_messages() -> MethodRouter<Service> {
    routing::get(
        |State(service): State<Service>, Path(strand): Path<String>| async move {
            Receipts(service).list_messages(strand).await
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/strands/{strand}/materials",
    summary = "Legacy render adapter: return a render view containing compiled skill text or the current assembled system prompt. The serialized view is not a recorded context-revision fragment or its byte count. This does not load, drop or query material packages.",
    params(("strand" = String, Path)),
    request_body = render::Request,
    responses(
        (status = 200, body = render::View),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn render() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(strand): Path<String>,
         Json(request): Json<render::Request>| async move {
            Receipts(service).render(strand, request).await
        },
    )
}
