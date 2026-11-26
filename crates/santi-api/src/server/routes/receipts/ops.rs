use super::*;
use santi_core::render;
use santi_core::{inference, receipt, soul, strand, webhook};

pub(super) struct Receipts(pub(super) Service);

impl Receipts {
    pub(super) async fn receipt(&self, inbox: String) -> Result<Json<receipt::Status>, Problem> {
        let receipt = self
            .0
            .receipt(&inbox)
            .await
            .map_err(Problem::service)?
            .ok_or_else(|| Problem::missing("receipt not found"))?;
        Ok(Json(receipt))
    }

    pub(super) async fn weave(
        &self,
        inference: Option<&inference::Declaration>,
    ) -> Result<Json<strand::Created>, Problem> {
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
    ) -> Result<Json<strand::Created>, Problem> {
        self.0
            .seating(&soul, inference)
            .await
            .map(Json)
            .map_err(super::super::inference::reject)
    }

    pub(super) async fn strands(&self) -> Result<Json<Vec<Strand>>, Problem> {
        self.0.strands().await.map(Json).map_err(Problem::service)
    }

    pub(super) async fn awaken(&self, request: soul::Draft) -> Result<Json<Soul>, Problem> {
        self.0
            .awaken(request)
            .await
            .map(Json)
            .map_err(super::super::inference::reject)
    }

    pub(super) async fn souls(&self) -> Result<Json<Vec<Soul>>, Problem> {
        self.0.souls().await.map(Json).map_err(Problem::service)
    }

    pub(super) async fn soul(&self, soul: String) -> Result<Json<Soul>, Problem> {
        match self.0.soul(&soul).await.map_err(Problem::service)? {
            Some(soul) => Ok(Json(soul)),
            None => Err(Problem::missing("soul not found")),
        }
    }

    pub(super) async fn subscribe(
        &self,
        request: webhook::Draft,
    ) -> Result<Json<webhook::Subscription>, Problem> {
        self.0
            .subscribe(request)
            .await
            .map(Json)
            .map_err(Problem::service)
    }

    pub(super) async fn webhooks(&self) -> Result<Json<Vec<webhook::Subscription>>, Problem> {
        self.0.webhooks().await.map(Json).map_err(Problem::service)
    }

    pub(super) async fn strand(&self, strand: String) -> Result<Json<strand::Detail>, Problem> {
        self.0
            .strand(&strand)
            .await
            .map_err(Problem::service)?
            .map(Json)
            .ok_or_else(|| Problem::missing("strand not found"))
    }

    pub(super) async fn messages(
        &self,
        strand: String,
    ) -> Result<Json<Vec<santi_core::message::Placed>>, Problem> {
        self.0
            .strand(&strand)
            .await
            .map_err(Problem::service)?
            .map(|detail| Json(detail.messages))
            .ok_or_else(|| Problem::missing("strand not found"))
    }

    pub(super) async fn render(
        &self,
        strand: String,
        request: render::Request,
    ) -> Result<Json<render::View>, Problem> {
        self.0
            .render(&strand, request)
            .await
            .map(Json)
            .map_err(Problem::service)
    }
}
