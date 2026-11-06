use super::{Service, receipt};
use receipt::reply::{Attempt, Owner, Resolution, Status};
use serde_json::Value;

impl Service {
    pub async fn reply(&self, inbox: &str) -> Result<Option<Status>, String> {
        self.store.window(inbox).await
    }
    pub async fn replies(&self) -> Result<Vec<Status>, String> {
        self.store.replies().await
    }
    pub async fn claim(
        &self,
        request: &receipt::reply::Request,
        inbox: &str,
        body: &Value,
    ) -> Result<(), String> {
        let status = self
            .store
            .window(inbox)
            .await?
            .ok_or("reply window not found")?;
        if status.strand != request.strand {
            return Err("reply receipt does not belong to the caller strand".into());
        }
        self.store
            .queue(
                inbox,
                santi_estate::Intent {
                    owner: Owner::Soul,
                    operation: &request.operation,
                    body,
                },
            )
            .await
    }
    pub async fn feedback(&self, inbox: &str, operation: &str, body: &Value) -> Result<(), String> {
        self.store
            .queue(
                inbox,
                santi_estate::Intent {
                    owner: Owner::Runtime,
                    operation,
                    body,
                },
            )
            .await
    }
    pub async fn outgoing(&self, inbox: &str) -> Result<Option<Attempt>, String> {
        self.store.dispatch(inbox).await
    }
    pub async fn delivered(
        &self,
        inbox: &str,
        result: santi_estate::Outcome<'_>,
    ) -> Result<(), String> {
        self.store.delivered(inbox, result).await
    }
    pub async fn resolution(&self, inbox: &str, resolution: &Resolution) -> Result<(), String> {
        self.store.resolution(inbox, resolution).await
    }
}
