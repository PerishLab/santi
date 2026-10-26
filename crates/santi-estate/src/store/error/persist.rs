use super::Seat;
use keel::Tx;
use keel::adapt::db::Sqlite;
use santi_error::{Incident, Mutation, Transition};

pub(super) struct Writer<'a, 'tx>(pub(super) &'a mut Tx<'tx, Sqlite>);

impl Writer<'_, '_> {
    pub(super) async fn opened(
        &mut self,
        seat: &Seat,
        active: Option<i64>,
        mutation: &Mutation,
    ) -> Result<(), keel::adapt::Error> {
        let incident = &mutation.incident;
        match (active, mutation.transition.as_ref()) {
            (Some(row), None) => self.latest(seat, row, incident).await,
            (None, Some(transition)) => self.put(seat, incident, transition).await,
            _ => Err(keel::adapt::Error::Adapt(
                "incident mutation disagrees with its active row".into(),
            )),
        }
    }

    pub(super) async fn resolved(
        &mut self,
        seat: &Seat,
        active: i64,
        mutation: &Mutation,
    ) -> Result<(), keel::adapt::Error> {
        let incident = &mutation.incident;
        let resolution = incident
            .resolution
            .as_ref()
            .ok_or_else(|| keel::adapt::Error::Adapt("resolution missing".into()))?;
        let transition = mutation
            .transition
            .as_ref()
            .ok_or_else(|| keel::adapt::Error::Adapt("resolution transition missing".into()))?;
        let context = json(&incident.latest.context)?;
        let payload = json(transition)?;
        let mut fields = vec![
            ("revision", incident.revision.to_string()),
            ("last_seen", incident.latest.seen.clone()),
            ("latest_context", context),
            ("resolved", resolution.at.clone()),
            ("resolved_event", transition.id.clone()),
            ("resolved_payload", payload),
        ];
        if let Some(by) = resolution.by.as_deref() {
            fields.push(("resolved_by", by.to_string()));
        }
        let fields = fields
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect::<Vec<_>>();
        self.0.set(seat.unit, active, &fields).await?;
        self.0.unset(seat.unit, active, &["active"]).await
    }

    async fn latest(
        &mut self,
        seat: &Seat,
        row: i64,
        incident: &Incident,
    ) -> Result<(), keel::adapt::Error> {
        let context = json(&incident.latest.context)?;
        self.0
            .set(
                seat.unit,
                row,
                &[
                    ("latest_source_component", &incident.latest.source.component),
                    ("latest_source_operation", &incident.latest.source.operation),
                    ("latest_message", &incident.latest.message),
                    ("latest_context", &context),
                    ("occurrence_count", &incident.occurrences.to_string()),
                    ("last_seen", &incident.latest.seen),
                ],
            )
            .await
    }

    async fn put(
        &mut self,
        seat: &Seat,
        incident: &Incident,
        transition: &Transition,
    ) -> Result<(), keel::adapt::Error> {
        let exposure = json(&incident.exposure)?;
        let first_context = json(&incident.first.context)?;
        let latest_context = json(&incident.latest.context)?;
        let payload = json(transition)?;
        let occurrences = incident.occurrences.to_string();
        let revision = incident.revision.to_string();
        let owner = seat.owner.to_string();
        self.0
            .put(
                seat.unit,
                &[
                    ("tag", incident.id.as_str()),
                    ("code", incident.code.as_str()),
                    ("active", incident.code.as_str()),
                    ("category", incident.category.db()),
                    ("severity", incident.severity.db()),
                    ("retry", incident.retry.db()),
                    ("exposure", exposure.as_str()),
                    ("source_component", incident.first.source.component.as_str()),
                    ("source_operation", incident.first.source.operation.as_str()),
                    (
                        "latest_source_component",
                        incident.latest.source.component.as_str(),
                    ),
                    (
                        "latest_source_operation",
                        incident.latest.source.operation.as_str(),
                    ),
                    ("message", incident.first.message.as_str()),
                    ("latest_message", incident.latest.message.as_str()),
                    ("context", first_context.as_str()),
                    ("latest_context", latest_context.as_str()),
                    ("occurrence_count", occurrences.as_str()),
                    ("revision", revision.as_str()),
                    ("first_seen", incident.first.seen.as_str()),
                    ("last_seen", incident.latest.seen.as_str()),
                    ("opened_event", transition.id.as_str()),
                    ("opened_payload", payload.as_str()),
                    (seat.field, owner.as_str()),
                ],
            )
            .await?;
        Ok(())
    }
}

fn json(value: &impl serde::Serialize) -> Result<String, keel::adapt::Error> {
    serde_json::to_string(value).map_err(|error| keel::adapt::Error::Adapt(error.to_string()))
}
