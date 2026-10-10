use santi_provider::Item;

use crate::service::Service;

impl Service {
    pub(super) async fn assembled(&self, strand: &str, turn: &str) -> Result<Vec<Item>, String> {
        let mut input = crate::provider_input(&self.store, strand).await?;
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
