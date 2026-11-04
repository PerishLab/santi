use super::*;

impl Service {
    pub async fn audit(&self, strand: &str) -> Result<Option<budget::Snapshot>, String> {
        let Some(strand) = self.store.strand(strand).await? else {
            return Ok(None);
        };
        let execution = self.rationed(&strand.id);
        let usage = if execution.is_some() {
            Some(self.usage(&strand.id).await?)
        } else {
            None
        };
        let composition = self.composition(&strand.id).await?;
        let estimate = self.weighed(&strand.id, composition.text).await?;
        let cap = self.budget();
        let ceiling = cap.as_ref().map(budget::Cap::ceiling);
        let pressure = if cap.is_some() {
            let gauge = self.gauge(&strand.id, true).await?;
            Some(budget::Pressure {
                basis: "ordinary_tools".into(),
                pending: true,
                active: gauge.active,
                ceiling: gauge
                    .ceiling
                    .ok_or("provider budget produced no active ceiling")?,
            })
        } else {
            None
        };
        Ok(Some(budget::Snapshot {
            strand: strand.id.clone(),
            request: budget::Request {
                basis: "offered_tools".into(),
                pending: true,
                estimate,
                ceiling,
            },
            pressure,
            materials: composition.components,
            budget: cap,
            slots: self.occupancy(&strand.id).await?,
            execution,
            usage,
            incident: self
                .store
                .incident(
                    &crate::budget::Error::Context
                        .descriptor()
                        .key("strand", &strand.id),
                )
                .await?,
        }))
    }
}
