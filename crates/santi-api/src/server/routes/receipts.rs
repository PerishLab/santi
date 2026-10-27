use super::*;
use santi_core::{material, receipt, soul, strand, webhook};

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
    responses((status = 200, body = strand::Created), (status = 500, body = Fault))
)]
pub(super) fn weave() -> MethodRouter<Service> {
    routing::post(|State(service): State<Service>| async move { Receipts(service).weave().await })
}

#[utoipa::path(
    post,
    path = "/api/v1/souls/{soul}/strands",
    params(("soul" = String, Path)),
    responses(
        (status = 200, body = strand::Created),
        (status = 400, body = Fault),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn seat() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>, Path(soul): Path<String>| async move {
            Receipts(service).seat(soul).await
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
    responses((status = 200, body = Soul), (status = 500, body = Fault))
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
    summary = "Render and return current material selected by kind for inspection. This request does not submit text or alter instructions or memory.",
    params(("strand" = String, Path)),
    request_body = material::Request,
    responses(
        (status = 200, body = material::Material),
        (status = 404, body = Fault),
        (status = 500, body = Fault)
    )
)]
pub(super) fn material() -> MethodRouter<Service> {
    routing::post(
        |State(service): State<Service>,
         Path(strand): Path<String>,
         Json(request): Json<material::Request>| async move {
            Receipts(service).material(strand, request).await
        },
    )
}

struct Receipts(Service);

impl Receipts {
    async fn receipt(&self, inbox: String) -> Result<Json<receipt::Status>, ApiError> {
        let receipt = self
            .0
            .receipt(&inbox)
            .await
            .map_err(ApiError::from_service)?
            .ok_or_else(|| ApiError::not_found("receipt not found"))?;
        Ok(Json(receipt))
    }

    async fn weave(&self) -> Result<Json<strand::Created>, ApiError> {
        self.0
            .weave()
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn seat(&self, soul: String) -> Result<Json<strand::Created>, ApiError> {
        self.0
            .seat(&soul)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn strands(&self) -> Result<Json<Vec<Strand>>, ApiError> {
        self.0
            .strands()
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn awaken(&self, request: soul::Draft) -> Result<Json<Soul>, ApiError> {
        self.0
            .awaken(request)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn souls(&self) -> Result<Json<Vec<Soul>>, ApiError> {
        self.0
            .souls()
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn get_soul(&self, soul: String) -> Result<Json<Soul>, ApiError> {
        match self.0.soul(&soul).await.map_err(ApiError::from_service)? {
            Some(soul) => Ok(Json(soul)),
            None => Err(ApiError::not_found("soul not found")),
        }
    }

    async fn subscribe(
        &self,
        request: webhook::Draft,
    ) -> Result<Json<webhook::Subscription>, ApiError> {
        self.0
            .subscribe(request)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn webhooks(&self) -> Result<Json<Vec<webhook::Subscription>>, ApiError> {
        self.0
            .webhooks()
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    async fn get_strand(&self, strand: String) -> Result<Json<strand::Detail>, ApiError> {
        self.0
            .strand(&strand)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("strand not found"))
    }

    async fn list_messages(
        &self,
        strand: String,
    ) -> Result<Json<Vec<santi_core::message::Placed>>, ApiError> {
        self.0
            .strand(&strand)
            .await
            .map_err(ApiError::from_service)?
            .map(|detail| Json(detail.messages))
            .ok_or_else(|| ApiError::not_found("strand not found"))
    }

    async fn material(
        &self,
        strand: String,
        request: material::Request,
    ) -> Result<Json<material::Material>, ApiError> {
        self.0
            .material(&strand, request)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }
}
