use super::{Service, now, receipt};

impl Service {
    pub async fn acknowledge(
        &self,
        inbox: &str,
        intent: &serde_json::Value,
    ) -> Result<bool, String> {
        self.store.acknowledge(inbox, intent, &now()).await
    }

    pub async fn acknowledged(
        &self,
        inbox: &str,
        result: &serde_json::Value,
    ) -> Result<(), String> {
        self.store.acknowledged(inbox, result, &now()).await
    }

    pub async fn confirmed(
        &self,
        inbox: &str,
        resolution: &receipt::Confirmation,
    ) -> Result<(), String> {
        self.store.confirmed(inbox, resolution, &now()).await
    }

    pub async fn acknowledgement(
        &self,
        inbox: &str,
    ) -> Result<Option<receipt::Acknowledgement>, String> {
        self.store.acknowledgement(inbox).await
    }
}
