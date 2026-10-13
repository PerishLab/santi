use super::*;
use crate::context::budget::estimated;
use santi_provider::Item;

impl Service {
    pub(in crate::service) async fn weight(
        &self,
        strand: &str,
        pending: bool,
    ) -> Result<budget::Estimate, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
        if pending {
            input.extend(self.pending_input(strand).await?);
        }
        let instructions = self.wording(strand).await?;
        Ok(estimated(
            &input,
            Some(&instructions),
            Some(&crate::service::tools::tools()),
        ))
    }

    pub(in crate::service) async fn estimate(
        &self,
        strand: &str,
    ) -> Result<budget::Estimate, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
        input.extend(self.pending_input(strand).await?);
        let instructions = self.wording(strand).await?;
        let tools = self.offered(strand).await?;
        Ok(estimated(&input, Some(&instructions), Some(&tools)))
    }

    pub(super) async fn pending_input(&self, strand: &str) -> Result<Vec<Item>, String> {
        Ok(self
            .store
            .inboxes(strand)
            .await?
            .into_iter()
            .filter_map(|inbox| {
                let content = inbox.content.rendered();
                (!content.trim().is_empty()).then(|| Item::Message {
                    role: match inbox.kind {
                        crate::message::Kind::Text => "user",
                        crate::message::Kind::SantiSystem => "system",
                    }
                    .to_string(),
                    content,
                })
            })
            .collect())
    }
}
