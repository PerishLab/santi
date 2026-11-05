use super::*;
use crate::context::budget::gauged;
use santi_provider::Item;

impl Service {
    pub(in crate::service) async fn weight(
        &self,
        provider: &dyn santi_provider::Provider,
        strand: &str,
        pending: bool,
    ) -> Result<budget::Estimate, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
        if pending {
            input.extend(self.pending_input(strand).await?);
        }
        let instructions = self.wording(strand).await?;
        self.measured(
            provider,
            input,
            instructions,
            crate::service::tools::tools(),
        )
    }

    pub(in crate::service) async fn estimate(
        &self,
        provider: &dyn santi_provider::Provider,
        strand: &str,
    ) -> Result<budget::Estimate, String> {
        self.weighed(provider, strand, self.wording(strand).await?)
            .await
    }

    pub(in crate::service) async fn weighed(
        &self,
        provider: &dyn santi_provider::Provider,
        strand: &str,
        instructions: String,
    ) -> Result<budget::Estimate, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
        input.extend(self.pending_input(strand).await?);
        self.measured(provider, input, instructions, self.offered(strand).await?)
    }

    pub(in crate::service) fn measured(
        &self,
        provider: &dyn santi_provider::Provider,
        input: Vec<Item>,
        instructions: String,
        tools: Vec<santi_provider::Tool>,
    ) -> Result<budget::Estimate, String> {
        let request = self.projected(provider, input, instructions, tools)?;
        Ok(gauged(&request, provider))
    }

    pub(in crate::service) fn projected(
        &self,
        provider: &dyn santi_provider::Provider,
        input: Vec<Item>,
        instructions: String,
        tools: Vec<santi_provider::Tool>,
    ) -> Result<santi_provider::Request, String> {
        let mut request = self.prepare(provider, input, instructions, tools)?;
        self.suffix(provider, &mut request);
        Ok(request)
    }

    pub(in crate::service) fn prepare(
        &self,
        provider: &dyn santi_provider::Provider,
        input: Vec<Item>,
        instructions: String,
        tools: Vec<santi_provider::Tool>,
    ) -> Result<santi_provider::Request, String> {
        provider.prepare(santi_provider::Request {
            model: provider.metadata().model,
            instructions: Some(instructions),
            input,
            tools: Some(tools),
            previous: None,
        })
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
