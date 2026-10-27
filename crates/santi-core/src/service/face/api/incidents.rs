use super::Service;
use crate::{History, Incident, Scope};

impl Service {
    pub async fn history(
        &self,
        scope: &Scope,
        limit: i64,
        before: Option<&str>,
    ) -> Result<Option<Vec<Incident>>, String> {
        self.store
            .history(scope, usize::try_from(limit).unwrap_or(usize::MAX), before)
            .await
    }

    pub async fn incident(&self, tag: &str) -> Result<Option<History>, String> {
        self.store.inspect(tag).await
    }
}
