use super::*;
use santi_core::render;
use santi_core::{inference, receipt, soul, strand, webhook};

pub(super) struct Receipts(pub(super) Service);

impl Receipts {
    pub(super) async fn receipt(&self, inbox: String) -> Result<Json<receipt::Status>, ApiError> {
        let receipt = self
            .0
            .receipt(&inbox)
            .await
            .map_err(ApiError::from_service)?
            .ok_or_else(|| ApiError::not_found("receipt not found"))?;
        Ok(Json(receipt))
    }

    pub(super) async fn weave(
        &self,
        inference: Option<&inference::Declaration>,
    ) -> Result<Json<strand::Created>, ApiError> {
        self.0
            .weaving(inference)
            .await
            .map(Json)
            .map_err(super::super::inference::reject)
    }

    pub(super) async fn seat(
        &self,
        soul: String,
        inference: Option<&inference::Declaration>,
    ) -> Result<Json<strand::Created>, ApiError> {
        self.0
            .seating(&soul, inference)
            .await
            .map(Json)
            .map_err(super::super::inference::reject)
    }

    pub(super) async fn strands(&self) -> Result<Json<Vec<Strand>>, ApiError> {
        self.0
            .strands()
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    pub(super) async fn awaken(&self, request: soul::Draft) -> Result<Json<Soul>, ApiError> {
        self.0
            .awaken(request)
            .await
            .map(Json)
            .map_err(super::super::inference::reject)
    }

    pub(super) async fn souls(&self) -> Result<Json<Vec<Soul>>, ApiError> {
        self.0
            .souls()
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    pub(super) async fn get_soul(&self, soul: String) -> Result<Json<Soul>, ApiError> {
        match self.0.soul(&soul).await.map_err(ApiError::from_service)? {
            Some(soul) => Ok(Json(soul)),
            None => Err(ApiError::not_found("soul not found")),
        }
    }

    pub(super) async fn subscribe(
        &self,
        request: webhook::Draft,
    ) -> Result<Json<webhook::Subscription>, ApiError> {
        self.0
            .subscribe(request)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    pub(super) async fn webhooks(&self) -> Result<Json<Vec<webhook::Subscription>>, ApiError> {
        self.0
            .webhooks()
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }

    pub(super) async fn get_strand(
        &self,
        strand: String,
    ) -> Result<Json<strand::Detail>, ApiError> {
        self.0
            .strand(&strand)
            .await
            .map_err(ApiError::from_service)?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("strand not found"))
    }

    pub(super) async fn list_messages(
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

    pub(super) async fn render(
        &self,
        strand: String,
        request: render::Request,
    ) -> Result<Json<render::View>, ApiError> {
        self.0
            .render(&strand, request)
            .await
            .map(Json)
            .map_err(ApiError::from_service)
    }
}
