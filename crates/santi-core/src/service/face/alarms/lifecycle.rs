use super::*;

impl Service {
    pub(in crate::service) async fn reconcile_alarms(&self) {
        let mut state = self.alarms.state.lock().await;
        if state.retry.is_some_and(|due| due <= Instant::now()) {
            self.apply_alarms(&mut state).await;
        }
    }

    pub(super) async fn apply_alarms(&self, state: &mut State) {
        let result = self.project_alarms().await;
        match result {
            Ok(mut projection) => {
                projection.observed = Some(crate::now());
                state.projection = projection;
                state.retry = Some(Instant::now() + Duration::from_secs(60));
            }
            Err(error) => {
                state.projection.observed = Some(crate::now());
                state.projection.applied = false;
                state.projection.error = Some(error);
                state.retry = Some(Instant::now() + Duration::from_secs(5));
            }
        }
    }

    async fn project_alarms(&self) -> Result<alarm::Projection, String> {
        let alarms = self.store.alarms().await?;
        let endpoint = self.alarms.endpoint.clone();
        tokio::task::spawn_blocking(move || endpoint.apply(&alarms))
            .await
            .map_err(|error| error.to_string())?
    }
}
