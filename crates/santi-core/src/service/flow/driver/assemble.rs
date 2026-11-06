use santi_provider::Item;

use crate::service::Service;

impl Service {
    pub(super) async fn assembled(&self, strand: &str, turn: &str) -> Result<Vec<Item>, String> {
        self.assembly(strand, turn, &[]).await
    }

    pub(super) async fn assembly(
        &self,
        strand: &str,
        turn: &str,
        pending: &[crate::message::Placed],
    ) -> Result<Vec<Item>, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
        input.extend(
            pending
                .iter()
                .filter_map(crate::assembly::timeline::projected),
        );
        if let Some(detail) = self.settlement(turn) {
            input.push(Item::Message {
                role: "system".to_string(),
                content: detail,
            });
        }
        let Some(detail) = self.crowded(strand).await? else {
            return Ok(input);
        };
        input.push(Item::Message {
            role: "system".to_string(),
            content: detail,
        });
        Ok(input)
    }
}
