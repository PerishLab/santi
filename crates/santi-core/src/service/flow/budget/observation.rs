use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::budget;
use crate::service::Service;
use crate::{Incident, strand};

const SCHEMA: &str = "santi.context.pending_observation.v1";
const REASON: &str = "pending_drain_would_exceed_budget";

#[derive(Serialize)]
pub(super) struct Observation {
    schema: &'static str,
    digest: String,
}

impl Observation {
    pub(super) fn matches(&self, incident: &Incident) -> bool {
        let context = &incident.latest.context;
        context["reason"].as_str() == Some(REASON)
            && context["details"]["observation"]["schema"].as_str() == Some(self.schema)
            && context["details"]["observation"]["digest"].as_str() == Some(self.digest.as_str())
    }
}

pub(super) async fn observe(
    service: &Service,
    strand: &strand::Strand,
    budget: &budget::Cap,
) -> Result<Observation, String> {
    let provider = service.provider.metadata();
    let pending = service
        .store
        .inboxes(&strand.id)
        .await?
        .into_iter()
        .map(|inbox| {
            json!({
                "id": inbox.id,
                "kind": inbox.kind,
                "content": inbox.content,
                "coalesce_revision": inbox.coalesce_revision,
            })
        })
        .collect::<Vec<_>>();
    let material = json!({
        "schema": SCHEMA,
        "estimator": crate::context::budget::ESTIMATOR,
        "strand": {
            "id": strand.id,
            "soul": strand.soul,
            "memory": strand.memory,
            "state": strand.state,
            "next": strand.next,
            "seen": strand.seen,
            "updated": strand.updated,
            "parent": strand.parent,
            "fork": strand.fork,
        },
        "pending": pending,
        "instructions": service.wording(&strand.id).await?,
        "tools": service.offered(&strand.id).await?,
        "provider": {
            "name": provider.provider.as_ref(),
            "model": provider.model,
            "budget": {"source": budget.source, "input": budget.bytes},
        },
    });
    let encoded = serde_json::to_vec(&material).map_err(|error| error.to_string())?;
    Ok(Observation {
        schema: SCHEMA,
        digest: format!("{:x}", Sha256::digest(encoded)),
    })
}
